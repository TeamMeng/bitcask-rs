#![allow(unused)]

use bytes::Bytes;

/// 写入到数据文件的记录，追加写入的数据类似日志的格式
pub struct LogRecord {
    pub(crate) key: Vec<u8>,
    pub(crate) value: Vec<u8>,
    pub(crate) rec_type: LogRecordType,
}

/// 数据位置索引信息，描述数据存储到了哪个位置
#[derive(Debug, Clone, Copy)]
pub struct LogRecordPos {
    /// 文件 id
    pub(crate) file_id: u32,
    /// 偏移量
    pub(crate) offset: u64,
}

/// 数据类型
#[derive(Debug, PartialEq, Eq)]
pub enum LogRecordType {
    /// 正常 Put 的数据
    Normal = 1,
    /// 被删除的数据标记，墓碑值
    Delete = 2,
}

impl LogRecord {
    pub fn new(key: Vec<u8>, value: Vec<u8>, rec_type: LogRecordType) -> Self {
        Self {
            key,
            value,
            rec_type,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        todo!()
    }
}

impl LogRecordPos {
    pub fn new(file_id: u32, offset: u64) -> Self {
        Self { file_id, offset }
    }
}
