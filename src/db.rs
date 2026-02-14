use crate::{
    data::{
        data_file::DataFile,
        log_record::{LogRecord, LogRecordPos, LogRecordType},
    },
    errors::AppError,
    index,
    options::Options,
};
use bytes::Bytes;
use parking_lot::RwLock;
use std::{collections::HashMap, sync::Arc};

/// 存储引擎实例结构体
pub struct Engine {
    /// 配置选项
    options: Arc<Options>,
    /// 当前活跃文件
    active_file: Arc<RwLock<DataFile>>,
    /// 旧数据文件
    older_file: Arc<RwLock<HashMap<u32, DataFile>>>,
    // 数据内存索引
    index: Box<dyn index::Indexer>,
}

impl Engine {
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
        let older_file = self.older_file.read();

        let log_record = match active_file.get_file_id() == log_record_pos.file_id {
            true => active_file.read_log_record(log_record_pos.offset)?,
            false => {
                if let Some(data_file) = older_file.get(&log_record_pos.file_id) {
                    data_file.read_log_record(log_record_pos.offset)?
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
            let mut older_files = self.older_file.write();
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
}
