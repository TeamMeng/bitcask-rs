use crate::{
    data::log_record::LogRecordPos,
    index::{IndexIterator, Indexer},
    options::IteratorOptions,
};
use bytes::Bytes;
use crossbeam_skiplist::SkipMap;
use std::sync::Arc;

/// 跳表索引
pub struct SkipList {
    skl: Arc<SkipMap<Vec<u8>, LogRecordPos>>,
}

/// 跳表索引迭代器
pub struct SkipListIterator {
    items: Vec<(Vec<u8>, LogRecordPos)>,
    curr_index: usize,
    options: IteratorOptions,
}

impl SkipList {
    pub fn new() -> Self {
        Self {
            skl: Arc::new(SkipMap::new()),
        }
    }
}

impl Indexer for SkipList {
    fn put(&self, key: Vec<u8>, pos: LogRecordPos) -> Option<LogRecordPos> {
        let mut result = None;
        if let Some(entry) = self.skl.get(&key) {
            result = Some(*entry.value());
        }
        self.skl.insert(key, pos);
        result
    }

    fn get(&self, key: &[u8]) -> Option<LogRecordPos> {
        if let Some(entry) = self.skl.get(key) {
            return Some(*entry.value());
        }
        None
    }

    fn delete(&self, key: &[u8]) -> Option<LogRecordPos> {
        if let Some(entry) = self.skl.remove(key) {
            return Some(*entry.value());
        }
        None
    }

    fn list_keys(&self) -> Vec<bytes::Bytes> {
        let mut keys = Vec::with_capacity(self.skl.len());

        for entry in self.skl.iter() {
            keys.push(Bytes::copy_from_slice(entry.key()));
        }

        keys
    }

    fn iterator(&self, opts: IteratorOptions) -> Box<dyn IndexIterator> {
        let mut items = Vec::with_capacity(self.skl.len());
        // 将 SkipList 中的数据存储到数组中
        for entry in self.skl.iter() {
            items.push((entry.key().clone(), *entry.value()));
        }
        if opts.reverse {
            items.reverse();
        }
        Box::new(SkipListIterator {
            items,
            curr_index: 0,
            options: opts,
        })
    }
}

impl IndexIterator for SkipListIterator {
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

    #[test]
    fn skl_should_work() {
        let skl = SkipList::new();

        // empty list
        let keys = skl.list_keys();
        assert!(keys.is_empty());

        // invalid delete
        let ret = skl.delete("aacd".as_bytes());
        assert!(ret.is_none());

        // invalid get
        let ret = skl.get("aacd".as_bytes());
        assert!(ret.is_none());

        skl.put("aacd".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));

        // valid get
        let ret = skl
            .get("aacd".as_bytes())
            .is_some_and(|pos| pos.file_id == 1123 && pos.offset == 1232);
        assert!(ret);

        // valid delete
        let ret = skl
            .delete("aacd".as_bytes())
            .is_some_and(|v| v.file_id == 1123 && v.offset == 1232);
        assert!(ret);

        // valid delete before get
        let ret = skl.get("aacd".as_bytes());
        assert!(ret.is_none());

        skl.put("acdd".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));
        skl.put("bbae".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));
        skl.put("ddee".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));

        // get list keys
        let keys = skl.list_keys();
        assert_eq!(
            vec![
                Bytes::from("acdd"),
                Bytes::from("bbae"),
                Bytes::from("ddee")
            ],
            keys
        );

        // new put
        skl.put("acdd".as_bytes().to_vec(), LogRecordPos::new(1, 1, 0));
        let ret = skl
            .get("acdd".as_bytes())
            .is_some_and(|v| v.file_id == 1 && v.offset == 1);
        assert!(ret);
    }

    #[test]
    fn skl_iterator_should_work() {
        let skl = SkipList::new();

        let mut iter = skl.iterator(IteratorOptions::default());

        // empty
        let ret = iter.next();
        assert!(ret.is_none());

        skl.put("aacd".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));
        skl.put("acdd".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));
        skl.put("bbae".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));
        skl.put("ddee".as_bytes().to_vec(), LogRecordPos::new(1123, 1232, 0));

        let mut iter = skl.iterator(IteratorOptions::default());

        let mut len = 0;
        while iter.next().is_some() {
            len += 1;
        }
        assert_eq!(len, 4);

        // seek next
        let mut iter = skl.iterator(IteratorOptions::default());
        iter.seek("aa".as_bytes());
        let ret = iter.next().is_some_and(|item| {
            item.0 == "aacd".as_bytes() && item.1.file_id == 1123 && item.1.offset == 1232
        });
        assert!(ret);

        // reverse
        let mut iter = skl.iterator(IteratorOptions {
            reverse: true,
            ..Default::default()
        });
        iter.seek("bb".as_bytes());
        let ret = iter.next().is_some_and(|item| {
            item.0 == "acdd".as_bytes() && item.1.file_id == 1123 && item.1.offset == 1232
        });
        assert!(ret);

        // prefix
        let mut iter = skl.iterator(IteratorOptions {
            prefix: "acdd".as_bytes().to_vec(),
            ..Default::default()
        });
        let ret = iter.next().is_some_and(|item| {
            item.0 == "acdd".as_bytes() && item.1.file_id == 1123 && item.1.offset == 1232
        });
        assert!(ret);
    }
}
