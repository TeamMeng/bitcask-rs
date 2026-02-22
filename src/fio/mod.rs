pub mod file_io;
pub mod mmap;

use crate::{
    errors::AppError,
    fio::{file_io::FileIO, mmap::MMapIo},
    options::IOType,
};
use std::path::PathBuf;

/// 抽象 IO 管理接口，可以接入不同的IO类型，目前支持标准文件 IO
pub trait IOManager: Sync + Send {
    /// 从文件的给定位置读取对应的数据
    fn read(&self, buf: &mut [u8], offset: u64) -> Result<usize, AppError>;

    /// 写入字节数组到文件中
    fn write(&self, buf: &[u8]) -> Result<usize, AppError>;

    /// 持久化数据
    fn sync(&self) -> Result<(), AppError>;

    /// 获取文件的大小
    fn size(&self) -> u64;
}

/// 根据文件名称初始化 IOManager。
/// `create`: true 表示创建/截断文件，false 表示打开已存在文件（mmap 时不会截断）
pub fn new_io_manager(file_name: PathBuf, io_type: IOType, create: bool) -> Box<dyn IOManager> {
    match io_type {
        IOType::StandardFIO => {
            Box::new(FileIO::new(file_name).expect("failed to new file io manager"))
        }
        IOType::MemoryMap => {
            let mmap = if create {
                MMapIo::new(file_name).expect("failed to new mmap")
            } else {
                MMapIo::open_existing(file_name).expect("failed to open existing mmap")
            };
            Box::new(mmap)
        }
    }
}
