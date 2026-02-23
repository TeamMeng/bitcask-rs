use fs_extra::dir::get_size;
use fs2::available_space;
use std::path::PathBuf;

/// 磁盘数据目录的大小
pub fn dir_dis_size(dir_path: PathBuf) -> u64 {
    get_size(dir_path).unwrap_or_default()
}

/// 获取磁盘剩余空间容量
pub fn available_disk_size() -> u64 {
    available_space(PathBuf::from("/")).unwrap_or_default()
}
