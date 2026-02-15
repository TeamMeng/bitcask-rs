use crate::{
    data::{
        data_file::{DATA_FILE_NAME_SUFFIX, DataFile},
        log_record::{LogRecord, LogRecordPos, LogRecordType},
    },
    errors::AppError,
    index::{self, new_indexer},
    options::Options,
};
use bytes::Bytes;
use log::warn;
use parking_lot::RwLock;
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

const INITIAL_FILE_ID: u32 = 0;

/// 存储引擎实例结构体
pub struct Engine {
    /// 配置选项
    options: Arc<Options>,
    /// 当前活跃文件
    active_file: Arc<RwLock<DataFile>>,
    /// 旧数据文件
    older_files: Arc<RwLock<HashMap<u32, DataFile>>>,
    /// 数据内存索引
    index: Box<dyn index::Indexer>,
    /// 数据库启动时的文件 id，只用于加载索引使用，不能在其他的地方更新或使用
    file_ids: Vec<u32>,
}

impl Engine {
    /// 打开 bitcask 存储引擎实例
    pub fn open(opts: Options) -> Result<Self, AppError> {
        // 检验用户传递过来的配置项
        check_options(&opts)?;

        // 判断数据目录是否存在，如果不存在则创建这个目录
        if !opts.dir_path.is_dir()
            && let Err(e) = fs::create_dir_all(&opts.dir_path)
        {
            warn!("failed to create the database dir: {}", e);
            return Err(AppError::FailedToCreateDatabaseDir);
        }

        // 加载数据文件
        let mut data_file = load_data_files(opts.dir_path.clone())?;

        // 设置 file_id 信息
        let mut file_ids = Vec::new();
        for v in data_file.iter() {
            file_ids.push(v.get_file_id());
        }

        // 将旧的数据文件保存到 older_files 中
        let mut older_files = HashMap::new();
        while let Some(file) = data_file.pop()
            && data_file.len() > 1
        {
            older_files.insert(file.get_file_id(), file);
        }

        // 拿到当前活跃文件
        let active_file = match data_file.pop() {
            Some(v) => v,
            None => DataFile::new(opts.dir_path.clone(), INITIAL_FILE_ID)?,
        };

        let index_type = opts.index_type.clone();

        // 构造存储引擎实例
        let engine = Self {
            options: Arc::new(opts),
            active_file: Arc::new(RwLock::new(active_file)),
            older_files: Arc::new(RwLock::new(older_files)),
            index: Box::new(new_indexer(index_type)),
            file_ids,
        };

        // 从数据文件中加载内存索引
        engine.load_index_from_data_files()?;

        Ok(engine)
    }

    /// 存储 key/value 数据，key不能为空
    pub fn put(&self, key: Bytes, value: Bytes) -> Result<(), AppError> {
        if key.is_empty() {
            return Err(AppError::KeyIsEmpty);
        }

        // 构造 LogRecord
        let log_record = LogRecord::new(key.to_vec(), value.to_vec(), LogRecordType::Normal);

        // 追加写到活跃数据文件中
        let log_record_pos = self.append_log_record(&log_record)?;

        // 更新内存索引
        self.index.put(key.to_vec(), log_record_pos);
        Ok(())
    }

    /// 根据 key 获取对应的数据
    pub fn get(&self, key: Bytes) -> Result<Bytes, AppError> {
        if key.is_empty() {
            return Err(AppError::KeyIsEmpty);
        }

        // 从内存索引中获取key，对应的数据信息
        let Some(log_record_pos) = self.index.get(&key) else {
            return Err(AppError::KeyNotFound);
        };

        let active_file = self.active_file.read();
        let older_file = self.older_files.read();

        let log_record = match active_file.get_file_id() == log_record_pos.file_id {
            true => active_file.read_log_record(log_record_pos.offset)?.record,
            false => {
                if let Some(data_file) = older_file.get(&log_record_pos.file_id) {
                    data_file.read_log_record(log_record_pos.offset)?.record
                } else {
                    return Err(AppError::DataFileNotFound);
                }
            }
        };

        // 判断 LogRecord 的类型
        if log_record.rec_type == LogRecordType::Delete {
            return Err(AppError::KeyNotFound);
        }

        // 返回对应的 value 信息
        Ok(log_record.value.into())
    }

    /// 追加写到活跃数据文件中
    fn append_log_record(&self, log_record: &LogRecord) -> Result<LogRecordPos, AppError> {
        let dir_path = self.options.dir_path.clone();
        // 输入数据进行解码
        let en_record = log_record.encode();
        let recode_len = en_record.len() as u64;

        // 获取当前活跃文件
        let mut active_file = self.active_file.write();
        // 判断当前活跃文件是否达到了阈值
        if active_file.get_write_off() + recode_len > self.options.data_file_size {
            active_file.sync()?;

            let current_file_id = active_file.get_file_id();
            // 添加到旧数据文件中
            let mut older_files = self.older_files.write();
            let old_file = DataFile::new(dir_path.clone(), current_file_id)?;
            older_files.insert(current_file_id, old_file);

            // 打开新的数据文件
            let new_active_file = DataFile::new(dir_path.clone(), current_file_id + 1)?;
            *active_file = new_active_file;
        }

        // 追加写数据到当前活跃文件中
        let write_off = active_file.get_write_off();
        active_file.write(&en_record)?;

        // 根据配置项决定是否持久化
        if self.options.sync_writes {
            active_file.sync()?;
        }

        // 构造数据索引信息
        Ok(LogRecordPos::new(active_file.get_file_id(), write_off))
    }

    /// 从数据文件中加载内存索引，遍历数据文件中的内存，并依次处理其中的记录
    fn load_index_from_data_files(&self) -> Result<(), AppError> {
        // 数据文件为空，直接返回
        if self.file_ids.is_empty() {
            return Ok(());
        }

        let active_file = self.active_file.read();
        let older_files = self.older_files.read();

        // 遍历每个文件 id，取出对应的数据文件，并加载其中的数据
        for (i, file_id) in self.file_ids.iter().enumerate() {
            let mut offset = 0;
            loop {
                let ret = match *file_id == active_file.get_file_id() {
                    true => active_file.read_log_record(offset),
                    false => {
                        if let Some(data_file) = older_files.get(file_id) {
                            data_file.read_log_record(offset)
                        } else {
                            return Err(AppError::FileNotFound);
                        }
                    }
                };

                let (log_record, size) = match ret {
                    Ok(res) => (res.record, res.size),
                    Err(e) => {
                        if e == AppError::ReadDataFileEOF {
                            break;
                        }
                        return Err(e);
                    }
                };

                // 构建内存索引
                let log_record_pos = LogRecordPos::new(*file_id, offset);

                match log_record.rec_type {
                    LogRecordType::Normal => {
                        self.index.put(log_record.key.to_vec(), log_record_pos)
                    }
                    LogRecordType::Delete => self.index.delete(&log_record.key),
                };

                // 递增 offset，下一次读取的时候从新的位置开始
                offset += size;
            }
            // 设置活跃文件的 offset
            if i == self.file_ids.len() - 1 {
                active_file.set_write_off(offset);
            }
        }

        Ok(())
    }
}

fn check_options(opts: &Options) -> Result<(), AppError> {
    if opts.dir_path.to_str().is_none_or(|size| size.is_empty()) {
        return Err(AppError::DirPathIsEmpty);
    }

    if opts.data_file_size == 0 {
        return Err(AppError::DataFileSizeTooSmall);
    }

    Ok(())
}

// 从数据目录中加载数据文件
fn load_data_files(dir_path: PathBuf) -> Result<Vec<DataFile>, AppError> {
    let Ok(dir) = fs::read_dir(&dir_path) else {
        return Err(AppError::FailedToReadDatabaseDir);
    };

    let mut file_ids = Vec::new();
    let mut data_files = Vec::new();

    for file in dir.flatten() {
        // 拿到文件名并且判断文件名是否以 .data 结尾
        if let Some(file_name) = file.file_name().to_str()
            && file_name.ends_with(DATA_FILE_NAME_SUFFIX)
        {
            let split_name = file_name.split(".").collect::<Vec<&str>>();
            let file_id = match split_name[0].parse::<u32>() {
                Ok(fid) => fid,
                Err(e) => {
                    warn!("the database directory maybe corrupted: {}", e);
                    return Err(AppError::DataDirectoryCorrupted);
                }
            };
            file_ids.push(file_id);
        }
    }

    // 如果没有数据文件，则直接返回
    if file_ids.is_empty() {
        return Ok(data_files);
    }
    // 对文件 id 进行排序，从小到大进行加载
    file_ids.sort();
    // 遍历所有文件 id，依次打开对应的数据文件
    for file_id in file_ids {
        data_files.push(DataFile::new(dir_path.clone(), file_id)?);
    }

    Ok(data_files)
}
