use crate::{
    data::log_record::{
        LogRecord, LogRecordPos, LogRecordType, ReadLogRecord, max_log_record_header_size,
    },
    errors::AppError,
    fio::{IOManager, new_io_manager},
    options::IOType,
};
use bytes::{Buf, BytesMut};
use parking_lot::RwLock;
use prost::{decode_length_delimiter, length_delimiter_len};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub const DATA_FILE_NAME_SUFFIX: &str = ".data";
pub const HINT_FILE_NAME: &str = "hint-index";
pub const MERGE_FINISHED_FILE_NAME: &str = "merge-finished";
pub const SEQ_NO_FILE_NAME: &str = "seq-no";

/// 数据文件
pub struct DataFile {
    /// 数据文件 id
    file_id: Arc<RwLock<u32>>,
    /// 写偏移，记录该数据文件写到哪个位置了
    write_off: Arc<RwLock<u64>>,
    /// IO 管理接口
    io_manager: Box<dyn IOManager>,
}

impl DataFile {
    /// 创建或打开一个数据文件。create=true 表示新建/截断，false 表示打开已存在文件（用于加载）
    pub fn new(dir_path: PathBuf, file_id: u32, io_type: IOType) -> Result<Self, AppError> {
        DataFile::new_with_create(dir_path, file_id, io_type, true)
    }

    /// 打开已存在的数据文件（加载时使用，mmap 不会截断）
    pub fn open(dir_path: PathBuf, file_id: u32, io_type: IOType) -> Result<Self, AppError> {
        DataFile::new_with_create(dir_path, file_id, io_type, false)
    }

    fn new_with_create(
        dir_path: PathBuf,
        file_id: u32,
        io_type: IOType,
        create: bool,
    ) -> Result<Self, AppError> {
        let file_name = get_data_file_name(&dir_path, file_id);
        let io_manager = new_io_manager(file_name, io_type, create);
        Ok(Self {
            file_id: Arc::new(RwLock::new(file_id)),
            write_off: Arc::new(RwLock::new(0)),
            io_manager,
        })
    }

    /// 新建或打开 hint 索引文件
    pub fn new_hint_file(dir_path: PathBuf) -> Result<DataFile, AppError> {
        let file_name = dir_path.join(HINT_FILE_NAME);
        let io_manager = new_io_manager(file_name, IOType::StandardFIO, true);
        Ok(Self {
            file_id: Arc::new(RwLock::new(0)),
            write_off: Arc::new(RwLock::new(0)),
            io_manager,
        })
    }

    /// 新建或打开 merge 完成的文件
    pub fn new_merge_fin_file(dir_path: PathBuf) -> Result<DataFile, AppError> {
        let file_name = dir_path.join(MERGE_FINISHED_FILE_NAME);
        let io_manager = new_io_manager(file_name, IOType::StandardFIO, true);
        Ok(Self {
            file_id: Arc::new(RwLock::new(0)),
            write_off: Arc::new(RwLock::new(0)),
            io_manager,
        })
    }

    /// 新建或打开存储事务序列号的文件
    pub fn new_seq_no_file(dir_path: PathBuf) -> Result<DataFile, AppError> {
        let file_name = dir_path.join(SEQ_NO_FILE_NAME);
        let io_manager = new_io_manager(file_name, IOType::StandardFIO, true);
        Ok(Self {
            file_id: Arc::new(RwLock::new(0)),
            write_off: Arc::new(RwLock::new(0)),
            io_manager,
        })
    }

    pub fn file_size(&self) -> u64 {
        self.io_manager.size()
    }

    pub fn get_write_off(&self) -> u64 {
        *self.write_off.read()
    }

    pub fn set_write_off(&self, offset: u64) {
        let mut write_guard = self.write_off.write();
        *write_guard = offset;
    }

    pub fn get_file_id(&self) -> u32 {
        *self.file_id.read()
    }

    /// 根据 offset 从数据文件中读取 LogRecord
    pub fn read_log_record(&self, offset: u64) -> Result<ReadLogRecord, AppError> {
        // 先读取出 header 部分的数据
        let mut header_buf = BytesMut::zeroed(max_log_record_header_size());

        self.io_manager.read(&mut header_buf, offset)?;

        // 取出 type，在第一个字节
        let rec_type = header_buf.get_u8();

        // 取出 key 和 value 的长度
        let Ok(key_size) = decode_length_delimiter(&mut header_buf) else {
            return Err(AppError::DecodeError);
        };
        let Ok(value_size) = decode_length_delimiter(&mut header_buf) else {
            return Err(AppError::DecodeError);
        };
        // 如果 key 和 value 都为0，则说明读取到文件的末尾，直接返回
        if key_size == 0 && value_size == 0 {
            return Err(AppError::ReadDataFileEOF);
        }

        // 获取实际的 key 和 value
        let actual_header_size =
            length_delimiter_len(key_size) + length_delimiter_len(value_size) + 1;

        // 读取实际的 key 和 value，最后的 4 个字节 crc
        let mut kv_buf = BytesMut::zeroed(key_size + value_size + 4);
        self.io_manager
            .read(&mut kv_buf, offset + actual_header_size as u64)?;

        let Some(key) = kv_buf.get(..key_size) else {
            return Err(AppError::GetValueFailed);
        };
        let Some(value) = kv_buf.get(key_size..kv_buf.len() - 4) else {
            return Err(AppError::GetValueFailed);
        };

        // 构造 LogRecord
        let log_record = LogRecord::new(
            key.to_vec(),
            value.to_vec(),
            LogRecordType::from_u8(rec_type),
        );

        // 向前移动到最后的 4 个字节 crc
        kv_buf.advance(key_size + value_size);
        if kv_buf.get_u32() != log_record.get_crc()? {
            return Err(AppError::InvalidLogRecordCrc);
        }

        // 构造 ReadLogRecord
        Ok(ReadLogRecord::new(
            log_record,
            actual_header_size + key_size + value_size + 4,
        ))
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, AppError> {
        if buf.is_empty() {
            return Err(AppError::KeyIsEmpty);
        }

        let n_bytes = self.io_manager.write(buf)?;
        let mut write_off = self.write_off.write();
        *write_off += n_bytes as u64;

        Ok(n_bytes)
    }

    /// 写 hint 索引到文件当中
    pub fn write_hint_record(&self, key: Vec<u8>, pos: LogRecordPos) -> Result<(), AppError> {
        let hint_record = LogRecord::new(key, pos.encode(), LogRecordType::Normal);
        let enc_record = hint_record.encode()?;
        self.write(&enc_record)?;

        Ok(())
    }

    pub fn sync(&self) -> Result<(), AppError> {
        self.io_manager.sync()
    }

    pub fn set_io_manager(&mut self, dir_path: PathBuf, io_type: IOType) {
        self.io_manager = new_io_manager(
            get_data_file_name(&dir_path, self.get_file_id()),
            io_type,
            true,
        )
    }
}

/// 获取文件名称
pub fn get_data_file_name(dir_path: &Path, file_id: u32) -> PathBuf {
    dir_path.join(format!("{:09}", file_id) + DATA_FILE_NAME_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::{env, fs};

    #[test]
    fn data_file_should_work() -> Result<()> {
        let dir_path = env::temp_dir();
        let data_file = DataFile::new(dir_path.clone(), 0, IOType::StandardFIO)?;
        assert_eq!(data_file.get_file_id(), 0);

        // 重复打开
        let data_file = DataFile::new(dir_path.clone(), 0, IOType::StandardFIO)?;
        assert_eq!(data_file.get_file_id(), 0);

        // 打开其他 id 的文件
        let data_file = DataFile::new(dir_path.clone(), 660, IOType::StandardFIO)?;
        assert_eq!(data_file.get_file_id(), 660);

        // valid write
        let size = data_file.write("aaa".as_bytes())?;
        assert_eq!(size, 3);
        let size = data_file.write("bb".as_bytes())?;
        assert_eq!(size, 2);

        // invalid write
        let ret = data_file
            .write("".as_bytes())
            .is_err_and(|e| e == AppError::KeyIsEmpty);
        assert!(ret);

        // sync
        data_file.sync()?;

        // read log record
        let enc = LogRecord::new(
            "name".as_bytes().to_vec(),
            "bitcask-rs".as_bytes().to_vec(),
            LogRecordType::Normal,
        )
        .encode()?;

        let size = data_file.write(&enc)?;
        assert_eq!(21, size);

        // 3 + 2
        let read_log_record = data_file.read_log_record(5)?;
        assert!(read_log_record.size == 21);
        assert_eq!(read_log_record.record.key, "name".as_bytes());
        assert_eq!(read_log_record.record.value, "bitcask-rs".as_bytes());
        assert_eq!(read_log_record.record.rec_type, LogRecordType::Normal);

        let enc = LogRecord::new(
            "a".as_bytes().to_vec(),
            "b".as_bytes().to_vec(),
            LogRecordType::Delete,
        )
        .encode()?;

        let size = data_file.write(&enc)?;
        assert_eq!(9, size);

        // 3 + 2 + 21
        let read_log_record = data_file.read_log_record(26)?;
        assert_eq!(read_log_record.record.key, "a".as_bytes());
        assert_eq!(read_log_record.record.value, "b".as_bytes());
        assert_eq!(read_log_record.record.rec_type, LogRecordType::Delete);

        // 删除生成文件
        fs::remove_file(get_data_file_name(&dir_path, 0))?;
        fs::remove_file(get_data_file_name(&dir_path, 660))?;
        Ok(())
    }
}
