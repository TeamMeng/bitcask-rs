use crate::{errors::AppError, fio::IOManager};
use log::error;
use memmap2::Mmap;
use parking_lot::Mutex;
use std::{fs::OpenOptions, path::PathBuf, sync::Arc};

pub struct MMapIo {
    map: Arc<Mutex<Mmap>>,
}

impl MMapIo {
    /// 创建或截断文件并以 mmap 方式打开（用于新文件）
    pub fn new(file_name: PathBuf) -> Result<Self, AppError> {
        match OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(file_name)
        {
            Ok(file) => {
                let map = unsafe { Mmap::map(&file).expect("failed to map file") };
                Ok(Self {
                    map: Arc::new(Mutex::new(map)),
                })
            }
            Err(e) => {
                error!("failed to open data file: {}", e);
                Err(AppError::FailedToOpenDataFile)
            }
        }
    }

    /// 打开已存在的文件并以 mmap 方式只读映射（不截断）
    pub fn open_existing(file_name: PathBuf) -> Result<Self, AppError> {
        match OpenOptions::new().read(true).open(file_name) {
            Ok(file) => {
                let map = unsafe { Mmap::map(&file).expect("failed to map file") };
                Ok(Self {
                    map: Arc::new(Mutex::new(map)),
                })
            }
            Err(e) => {
                error!("failed to open data file: {}", e);
                Err(AppError::FailedToOpenDataFile)
            }
        }
    }
}

impl IOManager for MMapIo {
    fn read(&self, buf: &mut [u8], offset: u64) -> Result<usize, AppError> {
        let map_arr = self.map.lock();
        let end = offset + buf.len() as u64;
        if end > map_arr.len() as _ {
            return Err(AppError::ReadDataFileEOF);
        }

        let val = &map_arr[offset as _..end as _];
        buf.copy_from_slice(val);

        Ok(val.len())
    }

    fn write(&self, _buf: &[u8]) -> Result<usize, AppError> {
        unimplemented!()
    }

    fn sync(&self) -> Result<(), AppError> {
        // MMap 是只读映射，不需要显式同步
        // 对于只读 mmap，内核会自动管理页面缓存
        Ok(())
    }

    fn size(&self) -> u64 {
        let map_arr = self.map.lock();
        map_arr.len() as _
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fio::file_io::FileIO;
    use anyhow::Result;
    use tempfile::NamedTempFile;

    #[test]
    fn mmap_should_work() -> Result<()> {
        let tmp = NamedTempFile::new()?;
        let file_name = tmp.path().to_path_buf();

        // 文件为空
        let mmap = MMapIo::new(file_name.clone())?;
        let fio = FileIO::new(file_name.clone())?;

        let mut buf = [0u8; 10];

        // empty
        let ret = mmap
            .read(&mut buf, 0)
            .is_err_and(|e| e == AppError::ReadDataFileEOF);
        assert!(ret);

        // size == 0
        assert_eq!(mmap.size(), 0);

        fio.write(b"aa")?;
        fio.write(b"bb")?;
        fio.write(b"cc")?;
        drop(fio); // 释放文件句柄，确保数据落盘后再 mmap

        // data：用 open_existing 打开已有文件，避免 truncate 清空内容
        let mmap = MMapIo::open_existing(file_name.clone())?;

        let mut buf = [0u8; 2];
        mmap.read(&mut buf, 0)?;
        assert_eq!(buf, "aa".as_bytes());

        mmap.read(&mut buf, 2)?;
        assert_eq!(buf, "bb".as_bytes());

        mmap.read(&mut buf, 4)?;
        assert_eq!(buf, "cc".as_bytes());

        // invalid data
        let ret = mmap
            .read(&mut buf, 7)
            .is_err_and(|e| e == AppError::ReadDataFileEOF);
        assert!(ret);

        assert_eq!(mmap.size(), 6);

        Ok(())
    }
}
