use crate::errors::AppError;
use fs_extra::dir::get_size;
use fs2::available_space;
use std::{fs, path::PathBuf};

/// 磁盘数据目录的大小
pub fn dir_dis_size(dir_path: PathBuf) -> u64 {
    get_size(dir_path).unwrap_or_default()
}

/// 获取磁盘剩余空间容量
pub fn available_disk_size() -> u64 {
    available_space(PathBuf::from("/")).unwrap_or_default()
}

pub fn copy_dir(src: PathBuf, dest: PathBuf, exclude: &[&str]) -> Result<(), AppError> {
    if !dest.exists() && fs::create_dir_all(&dest).is_err() {
        return Err(AppError::FailedToCreateDatabaseDir);
    }

    let Ok(dir) = fs::read_dir(src) else {
        return Err(AppError::FailedToReadDatabaseDir);
    };

    for dir_entry in dir {
        let Ok(entry) = dir_entry else {
            return Err(AppError::FailedToReadDatabaseDir);
        };

        let src_path = entry.path();
        if exclude.iter().any(|&x| src_path.ends_with(x)) {
            continue;
        }

        let des_path = dest.join(entry.file_name());
        if entry.file_type().expect("failed to get file type").is_dir() {
            copy_dir(src_path, des_path, exclude)?;
        } else if fs::copy(src_path, des_path).is_err() {
            return Err(AppError::FailedToCopyDir);
        }
    }

    Ok(())
}
