pub mod btree;

use crate::{
    data::log_record::LogRecordPos,
    index::btree::BTree,
    options::{IndexType, IteratorOptions},
};
use bytes::Bytes;

/// 抽象索引接口，如果想要接入其他的数据结构，直接实现这个接口即可
pub trait Indexer: Sync + Send {
    /// 向索引中存储 key 对应的数据位置信息
    fn put(&self, key: Vec<u8>, pos: LogRecordPos) -> bool;

    /// 根据 key 取出对应的索引位置信息
    fn get(&self, key: &[u8]) -> Option<LogRecordPos>;

    /// 根据 key 删除对应的索引位置信息
    fn delete(&self, key: &[u8]) -> bool;

    /// 获取索引存储的所有 key
    fn list_keys(&self) -> Vec<Bytes>;

    /// 返回索引迭代器
    fn iterator(&self, opts: IteratorOptions) -> Box<dyn IndexIterator>;
}

/// 抽象索引迭代器
pub trait IndexIterator: Sync + Send {
    /// 重新回到迭代器的起点，迭代器会从第一个元素开始迭代
    fn rewind(&mut self);

    /// 根据传入的key，查找第一个大于（或小于）等于的目标key，根据这个key开始遍历
    fn seek(&mut self, key: &[u8]);

    /// 跳转下一个 key，返回 None说明迭代完毕
    fn next(&mut self) -> Option<(&[u8], &LogRecordPos)>;
}

/// 根据类型打开内存索引
pub fn new_indexer(index_type: IndexType) -> impl Indexer {
    match index_type {
        IndexType::BTree => BTree::new(),
        IndexType::SkipList => todo!(),
    }
}
