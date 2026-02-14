#![allow(unused)]

use crate::{errors::AppError, fio::IOManager};
use log::error;
use parking_lot::RwLock;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::unix::fs::FileExt,
    path::PathBuf,
    sync::Arc,
};

/// 标准系统文件 IO
pub struct FileIO {
    /// 系统文件描述符
    fd: Arc<RwLock<File>>,
}

impl FileIO {
    pub fn new(file_name: PathBuf) -> Result<Self, AppError> {
        match OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(file_name)
        {
            Ok(file) => Ok(Self {
                fd: Arc::new(RwLock::new(file)),
            }),
            Err(e) => {
                error!("failed to open data file: {}", e);
                Err(AppError::Open)
            }
        }
    }
}

impl IOManager for FileIO {
    fn read(&self, buf: &mut [u8], offset: u64) -> Result<usize, AppError> {
        match self.fd.read().read_at(buf, offset) {
            Ok(size) => Ok(size),
            Err(e) => {
                error!("read from data file err: {}", e);
                Err(AppError::Read)
            }
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize, AppError> {
        let mut write_guard = self.fd.write();
        match write_guard.write(buf) {
            Ok(size) => Ok(size),
            Err(e) => {
                error!("write to data file err: {}", e);
                Err(AppError::Write)
            }
        }
    }

    fn sync(&self) -> Result<(), AppError> {
        match self.fd.read().sync_data() {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("write to data file err: {:?}", e);
                Err(AppError::Sync)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::fs;

    #[test]
    fn file_io_should_work() -> Result<()> {
        let file_name = PathBuf::from("/tmp/a.data");
        let fio = FileIO::new(file_name.clone())?;

        // write
        fio.write("key-a".as_bytes())?;

        // read
        let mut buf = [0u8; 5];
        let size = fio.read(&mut buf, 0)?;
        assert_eq!(5, size);

        // write
        fio.write("key-b".as_bytes())?;

        // read
        let size = fio.read(&mut buf, 5)?;
        assert_eq!(5, size);

        fio.sync()?;

        // read
        let mut buf = [0u8; 5];
        let size = fio.read(&mut buf, 5)?;
        assert_eq!(5, size);

        // read
        let size = fio.read(&mut buf, 0)?;
        assert_eq!(5, size);

        fs::remove_file(file_name)?;
        Ok(())
    }
}
