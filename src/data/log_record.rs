#![allow(unused)]

/// 数据位置索引信息，描述数据存储到了哪个位置
#[derive(Debug, Clone, Copy)]
pub struct LogRecordPos {
    /// 文件 id
    pub(crate) file_id: u32,
    /// 偏移量
    pub(crate) offset: u64,
}

impl LogRecordPos {
    pub fn new(file_id: u32, offset: u64) -> Self {
        Self { file_id, offset }
    }
}
