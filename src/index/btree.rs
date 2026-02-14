#![allow(unused)]

use crate::{data::log_record::LogRecordPos, index::Indexer};
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
}
