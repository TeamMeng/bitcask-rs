#![allow(unused)]

use crate::{data::log_record::ReadLogRecord, errors::AppError, fio::IOManager};
use parking_lot::RwLock;
use std::{path::PathBuf, sync::Arc};

pub const DATA_FILE_NAME_SUFFIX: &str = ".data";

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

    pub fn set_write_off(&self, offset: u64) {
        let mut write_guard = self.write_off.write();
        *write_guard = offset;
    }

    pub fn get_file_id(&self) -> u32 {
        *self.file_id.read()
    }

    pub fn read_log_record(&self, offset: u64) -> Result<ReadLogRecord, AppError> {
        todo!()
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, AppError> {
        todo!()
    }

    pub fn sync(&self) -> Result<(), AppError> {
        todo!()
    }
}
