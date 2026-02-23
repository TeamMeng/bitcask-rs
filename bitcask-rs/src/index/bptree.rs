use crate::{
    data::log_record::{LogRecordPos, decode_log_record_pos},
    index::{IndexIterator, Indexer},
    options::IteratorOptions,
};
use bytes::Bytes;
use jammdb::DB;
use log::error;
use std::{path::PathBuf, sync::Arc};

const BPTREE_INDEX_FILE_NAME: &str = "bptree-index";
const BPTREE_BUCKET_NAME: &str = "bitcask-index";

/// B+ 树索引
pub struct BPlusTree {
    tree: Arc<DB>,
}

impl BPlusTree {
    pub fn new(dir_path: PathBuf) -> Self {
        // 打开 B+ 树，并创建对应的 bucket
        let bptree =
            DB::open(dir_path.join(BPTREE_INDEX_FILE_NAME)).expect("failed to open bptree");
        let tree = Arc::new(bptree);
        let tx = tree.tx(true).expect("failed to begin tx");
        tx.get_or_create_bucket(BPTREE_BUCKET_NAME)
            .expect("failed to get or create bucket");
        tx.commit().expect("failed to commit");

        Self { tree }
    }
}

impl Indexer for BPlusTree {
    fn put(&self, key: Vec<u8>, pos: LogRecordPos) -> Option<LogRecordPos> {
        let tx = self.tree.tx(true).expect("failed to begin tx");
        let Ok(bucket) = tx.get_bucket(BPTREE_BUCKET_NAME) else {
            error!("failed to get bucket");
            return None;
        };

        let mut ret = None;
        if let Some(kv) = bucket.get_kv(&key) {
            ret = Some(decode_log_record_pos(kv.value().to_vec()));
        }

        // put 新值
        bucket
            .put(key, pos.encode())
            .expect("failed to put value in bptree");

        if tx.commit().is_err() {
            error!("failed to commit");
            return None;
        }

        ret
    }

    fn get(&self, key: &[u8]) -> Option<LogRecordPos> {
        let tx = self.tree.tx(false).expect("failed to begin tx");
        let Ok(bucket) = tx.get_bucket(BPTREE_BUCKET_NAME) else {
            error!("failed to get bucket");
            return None;
        };

        if let Some(kv) = bucket.get_kv(key) {
            return Some(decode_log_record_pos(kv.value().to_vec()));
        }

        None
    }

    fn delete(&self, key: &[u8]) -> Option<LogRecordPos> {
        let tx = self.tree.tx(true).expect("failed to begin tx");
        let Ok(bucket) = tx.get_bucket(BPTREE_BUCKET_NAME) else {
            error!("failed to get bucket");
            return None;
        };

        let mut ret = None;

        if let Ok(kv) = bucket.delete(key) {
            ret = Some(decode_log_record_pos(kv.value().to_vec()));
        } else {
            return ret;
        }

        if tx.commit().is_err() {
            error!("failed to commit");
            return None;
        }

        ret
    }

    fn list_keys(&self) -> Vec<bytes::Bytes> {
        let tx = self.tree.tx(false).expect("failed to begin tx");
        let mut keys = Vec::new();
        let Ok(bucket) = tx.get_bucket(BPTREE_BUCKET_NAME) else {
            error!("failed to get bucket");
            return keys;
        };

        for data in bucket.cursor() {
            keys.push(Bytes::copy_from_slice(data.key()));
        }

        keys
    }

    fn iterator(&self, opts: IteratorOptions) -> Box<dyn IndexIterator> {
        let tx = self.tree.tx(false).expect("failed to begin tx");
        let bucket = tx
            .get_bucket(BPTREE_BUCKET_NAME)
            .expect("failed to get bucket");

        let mut items = Vec::new();
        for data in bucket.cursor() {
            let key = data.key().to_vec();
            let pos = decode_log_record_pos(data.kv().value().to_vec());
            items.push((key, pos));
        }

        if opts.reverse {
            items.reverse();
        }

        Box::new(BPlusTreeIterator {
            items,
            curr_index: 0,
            options: opts,
        })
    }
}

pub struct BPlusTreeIterator {
    items: Vec<(Vec<u8>, LogRecordPos)>,
    curr_index: usize,
    options: IteratorOptions,
}

impl IndexIterator for BPlusTreeIterator {
    fn rewind(&mut self) {
        self.curr_index = 0
    }

    fn seek(&mut self, key: &[u8]) {
        self.curr_index = match self.items.binary_search_by(|(x, _)| {
            if self.options.reverse {
                x.cmp(&key.to_vec()).reverse()
            } else {
                x.cmp(&key.to_vec())
            }
        }) {
            Ok(equal_val) => equal_val,
            Err(insert_val) => insert_val,
        };
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
    use anyhow::Result;
    use std::fs;

    #[test]
    fn bptree_should_work() -> Result<()> {
        let dir_path = PathBuf::from("/tmp/bptree");
        fs::create_dir_all(dir_path.clone())?;
        let bptree = BPlusTree::new(dir_path.clone());

        // empty list
        let keys = bptree.list_keys();
        assert!(keys.is_empty());

        // invalid delete
        let ret = bptree.delete("aacd".as_bytes());
        assert!(ret.is_none());

        // invalid get
        let ret = bptree.get("aacd".as_bytes());
        assert!(ret.is_none());

        bptree.put("aacd".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));

        // valid get
        let ret = bptree
            .get("aacd".as_bytes())
            .is_some_and(|pos| pos.file_id == 1123 && pos.offset == 1232);
        assert!(ret);

        // valid delete
        let ret = bptree
            .delete("aacd".as_bytes())
            .is_some_and(|v| v.file_id == 1123 && v.offset == 1232);
        assert!(ret);

        // valid delete before get
        let ret = bptree.get("aacd".as_bytes());
        assert!(ret.is_none());

        bptree.put("acdd".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));
        bptree.put("bbae".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));
        bptree.put("ddee".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));

        // get list keys
        let keys = bptree.list_keys();
        assert_eq!(
            vec![
                Bytes::from("acdd"),
                Bytes::from("bbae"),
                Bytes::from("ddee")
            ],
            keys
        );

        // new put
        bptree.put("acdd".as_bytes().to_vec(), LogRecordPos::new(1, 1, 0));
        let ret = bptree
            .get("acdd".as_bytes())
            .is_some_and(|v| v.file_id == 1 && v.offset == 1 && v.size == 0);
        assert!(ret);

        fs::remove_dir_all(dir_path)?;
        Ok(())
    }

    #[test]
    fn bptree_iterator_should_work() -> Result<()> {
        let dir_path = PathBuf::from("/tmp/bptree-iterator");
        fs::create_dir_all(dir_path.clone())?;
        let bptree = BPlusTree::new(dir_path.clone());

        let mut iter = bptree.iterator(IteratorOptions::default());

        // empty
        let ret = iter.next();
        assert!(ret.is_none());

        bptree.put("aacd".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));
        bptree.put("acdd".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));
        bptree.put("bbae".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));
        bptree.put("ddee".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));

        let mut iter = bptree.iterator(IteratorOptions::default());

        let mut len = 0;
        while iter.next().is_some() {
            len += 1;
        }
        assert_eq!(len, 4);

        // seek next
        let mut iter = bptree.iterator(IteratorOptions::default());
        iter.seek("aa".as_bytes());
        let ret = iter.next().is_some_and(|item| {
            item.0 == "aacd".as_bytes() && item.1.file_id == 1123 && item.1.offset == 1232
        });
        assert!(ret);

        // reverse
        let mut iter = bptree.iterator(IteratorOptions {
            reverse: true,
            ..Default::default()
        });
        iter.seek("bb".as_bytes());
        let ret = iter.next().is_some_and(|item| {
            item.0 == "acdd".as_bytes() && item.1.file_id == 1123 && item.1.offset == 1232
        });
        assert!(ret);

        // prefix
        let mut iter = bptree.iterator(IteratorOptions {
            prefix: "acdd".as_bytes().to_vec(),
            ..Default::default()
        });
        let ret = iter.next().is_some_and(|item| {
            item.0 == "acdd".as_bytes() && item.1.file_id == 1123 && item.1.offset == 1232
        });

        assert!(ret);

        fs::remove_dir_all(dir_path)?;
        Ok(())
    }
}
