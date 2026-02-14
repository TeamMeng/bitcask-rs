use std::path::PathBuf;

/// 数据库启动配置项
pub struct Options {
    /// 数据库目录
    pub dir_path: PathBuf,
    /// 数据文件大小
    pub data_file_size: u64,
    /// 是否每次写入时持久化
    pub sync_writes: bool,
}
