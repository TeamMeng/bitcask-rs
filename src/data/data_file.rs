#![allow(unused)]

use crate::{data::log_record::LogRecord, errors::AppError, fio::IOManager};
use parking_lot::RwLock;
use std::{path::PathBuf, sync::Arc};

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
    pub fn new(dir_path: PathBuf, file_id: u32) -> Result<Self, AppError> {
        todo!()
    }

    pub fn get_write_off(&self) -> u64 {
        *self.write_off.read()
    }

    pub fn get_file_id(&self) -> u32 {
        *self.file_id.read()
    }

    pub fn read_log_record(&self, offset: u64) -> Result<LogRecord, AppError> {
        todo!()
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, AppError> {
        todo!()
    }

    pub fn sync(&self) -> Result<(), AppError> {
        todo!()
    }
}
