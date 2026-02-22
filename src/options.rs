use std::{env, path::PathBuf};

/// 数据库启动配置项
#[derive(Debug, Clone)]
pub struct Options {
    /// 数据库目录
    pub dir_path: PathBuf,
    /// 数据文件大小
    pub data_file_size: u64,
    /// 累计写到多少字节后持久化
    pub bytes_per_sync: usize,
    /// 是否每次写入时持久化
    pub sync_writes: bool,
    /// 数据库类型
    pub index_type: IndexType,
    /// 是否用 mmap 打开数据库
    pub mmap_at_startup: bool,
}

/// 索引迭代器配置项
#[derive(Default)]
pub struct IteratorOptions {
    // 前缀
    pub prefix: Vec<u8>,
    // 是否反向
    pub reverse: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexType {
    /// BTree 索引
    BTree,

    /// 跳表索引
    SkipList,

    // B+树索引
    BPlusTree,
}

#[derive(Clone, PartialEq)]
pub enum IOType {
    // 标准文件 IO
    StandardFIO,
    // 内存文件映射
    MemoryMap,
}

/// 批量写数据配置项
pub struct WriteBatchOptions {
    /// 一次批次当中的最大数据量
    pub max_batch_num: usize,
    /// 提交时是否进行 sync 持久化
    pub sync_writes: bool,
}

impl Default for WriteBatchOptions {
    fn default() -> Self {
        Self {
            max_batch_num: 10000,
            sync_writes: true,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self {
            dir_path: env::temp_dir().join("bitcask-rs"),
            data_file_size: 256 * 1024 * 1024,
            bytes_per_sync: 0,
            sync_writes: false,
            index_type: IndexType::BTree,
            mmap_at_startup: true,
        }
    }
}
