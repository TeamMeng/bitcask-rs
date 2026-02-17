use crate::{
    data::log_record::LogRecordPos,
    index::{IndexIterator, Indexer},
    options::IteratorOptions,
};
use bytes::Bytes;
use parking_lot::RwLock;
use std::{collections::BTreeMap, sync::Arc};

// BTree 索引，主要封装了标准库中的 BTreeMap 结构
pub struct BTree {
    tree: Arc<RwLock<BTreeMap<Vec<u8>, LogRecordPos>>>,
}

impl BTree {
    pub fn new() -> Self {
        Self {
            tree: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

impl Indexer for BTree {
    fn put(&self, key: Vec<u8>, pos: LogRecordPos) -> bool {
        self.tree.write().insert(key, pos);
        true
    }

    fn get(&self, key: &[u8]) -> Option<LogRecordPos> {
        self.tree.read().get(key).copied()
    }

    fn delete(&self, key: &[u8]) -> bool {
        // 存在 true，则 false 无效数据
        self.tree.write().remove(key).is_some()
    }

    fn list_keys(&self) -> Vec<Bytes> {
        let read_guard = self.tree.read();
        let mut keys = Vec::with_capacity(read_guard.len());

        for (k, _) in read_guard.iter() {
            keys.push(Bytes::copy_from_slice(k));
        }

        keys
    }

    fn iterator(&self, opts: IteratorOptions) -> Box<dyn IndexIterator> {
        let read_guard = self.tree.read();
        let mut items = Vec::with_capacity(read_guard.len());
        // 将 BTree 中的数据存储都数组中
        for (key, value) in read_guard.iter() {
            items.push((key.clone(), *value));
        }
        if opts.reverse {
            items.reverse();
        }
        Box::new(BTreeIterator {
            items,
            curr_index: 0,
            options: opts,
        })
    }
}

/// BTree 索引迭代器
pub struct BTreeIterator {
    // 存储 key和索引
    items: Vec<(Vec<u8>, LogRecordPos)>,
    // 当前遍历的位置下标
    curr_index: usize,
    // 索引迭代器配置项
    options: IteratorOptions,
}

impl IndexIterator for BTreeIterator {
    fn rewind(&mut self) {
        self.curr_index = 0;
    }

    fn seek(&mut self, key: &[u8]) {
        self.curr_index = match self.items.binary_search_by(|(vec, _)| {
            if self.options.reverse {
                vec.cmp(&key.to_vec()).reverse()
            } else {
                vec.cmp(&key.to_vec())
            }
        }) {
            Ok(equal_val) => equal_val,
            Err(insert_val) => insert_val,
        }
    }

    fn next(&mut self) -> Option<(&[u8], &LogRecordPos)> {
        if self.curr_index >= self.items.len() {
            return None;
        }

        while let Some(item) = self.items.get(self.curr_index) {
            self.curr_index += 1;
            let prefix = &self.options.prefix;
            if prefix.is_empty() || item.0.starts_with(prefix) {
                return Some((&item.0, &item.1));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn btree_should_work() {
        let btree = BTree::new();

        // invalid get
        let ret = btree.get("non exists".as_bytes());
        assert!(ret.is_none());
        // invalid delete
        let ret = btree.delete("non exists".as_bytes());
        assert!(!ret);

        // put
        btree.put("111".as_bytes().to_vec(), LogRecordPos::new(1, 10));

        // valid get
        let ret = btree.get("111".as_bytes()).is_some_and(|log_record_pos| {
            log_record_pos.file_id == 1 && log_record_pos.offset == 10
        });
        assert!(ret);

        // valid delete
        let ret = btree.delete("111".as_bytes());
        assert!(ret);
    }

    #[test]
    fn btree_iterator_should_work() {
        let bt = BTree::new();

        // 没有数据的情况
        let mut iter = bt.iterator(IteratorOptions::default());
        iter.seek("aa".as_bytes());

        let ret = iter.next();
        assert!(ret.is_none());

        // 有一条数据的情况
        bt.put("ccde".as_bytes().to_vec(), LogRecordPos::new(1, 10));
        let mut iter = bt.iterator(IteratorOptions::default());
        // seek 一个比 ccde 小的值
        iter.seek("aa".as_bytes());
        let ret = iter.next().is_some_and(|(vec, pos)| {
            vec == "ccde".as_bytes().to_vec() && pos.file_id == 1 && pos.offset == 10
        });
        assert!(ret);
        // seek 一个比 ccde 大的值
        iter.rewind();
        iter.seek("dd".as_bytes());
        let ret = iter.next().is_none();
        assert!(ret);

        // 多条数据
        bt.put("bbed".as_bytes().to_vec(), LogRecordPos::new(1, 10));
        bt.put("aaed".as_bytes().to_vec(), LogRecordPos::new(1, 10));
        bt.put("cadd".as_bytes().to_vec(), LogRecordPos::new(1, 10));

        let mut iter = bt.iterator(IteratorOptions::default());
        iter.seek("b".as_bytes());

        let mut size = 0;
        while iter.next().is_some() {
            size += 1;
        }
        assert!(size == 3);

        // seek 是cadd开头
        let mut iter = bt.iterator(IteratorOptions::default());
        iter.seek("cadd".as_bytes());
        let ret = iter.next().is_some_and(|(vec, pos)| {
            vec == "cadd".as_bytes().to_vec() && pos.file_id == 1 && pos.offset == 10
        });
        assert!(ret);

        // 都要大的值
        let mut iter = bt.iterator(IteratorOptions::default());
        iter.seek("zzz".as_bytes());
        assert!(iter.next().is_none());

        // 反向迭代
        let mut iter = bt.iterator(IteratorOptions {
            reverse: true,
            ..Default::default()
        });
        iter.seek("zzz".as_bytes());
        let mut size = 0;
        while iter.next().is_some() {
            size += 1;
        }
        assert_eq!(4, size);

        // 前缀 cadd
        let mut iter = bt.iterator(IteratorOptions {
            prefix: "cadd".as_bytes().to_vec(),
            ..Default::default()
        });
        let ret = iter.next().is_some_and(|(vec, pos)| {
            vec == "cadd".as_bytes().to_vec() && pos.file_id == 1 && pos.offset == 10
        });
        assert!(ret);
        assert!(iter.next().is_none());
    }
}
