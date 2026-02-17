use std::{env, path::PathBuf};

/// 数据库启动配置项
#[derive(Debug, Clone)]
pub struct Options {
    /// 数据库目录
    pub dir_path: PathBuf,
    /// 数据文件大小
    pub data_file_size: u64,
    /// 是否每次写入时持久化
    pub sync_writes: bool,
    /// 数据库类型
    pub index_type: IndexType,
}

/// 索引迭代器配置项
#[derive(Default)]
pub struct IteratorOptions {
    // 前缀
    pub prefix: Vec<u8>,
    // 是否反向
    pub reverse: bool,
}

#[derive(Debug, Clone)]
pub enum IndexType {
    /// BTree 索引
    BTree,

    /// 跳表索引
    SkipList,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            dir_path: env::temp_dir().join("bitcask-rs"),
            data_file_size: 256 * 1024 * 1024,
            sync_writes: false,
            index_type: IndexType::BTree,
        }
    }
}
