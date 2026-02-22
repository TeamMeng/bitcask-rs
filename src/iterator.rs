use crate::{db::Engine, index::IndexIterator, options::IteratorOptions};
use bytes::Bytes;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct Iterator<'a> {
    index_iter: Arc<RwLock<Box<dyn IndexIterator>>>,
    engine: &'a Engine,
}

impl Engine {
    /// 获取迭代器
    pub fn iter(&self, options: IteratorOptions) -> Iterator<'_> {
        Iterator {
            index_iter: Arc::new(RwLock::new(self.index.iterator(options))),
            engine: self,
        }
    }

    /// 返回数据中的所有key
    pub fn list_keys(&self) -> Vec<Bytes> {
        self.index.list_keys()
    }

    /// 对数据库中的所有数据执行函数操作，函数返回false时终止
    pub fn flod<F>(&self, f: F)
    where
        Self: Sized,
        F: Fn(Bytes, Bytes) -> bool,
    {
        let iter = self.iter(IteratorOptions::default());
        while let Some((key, value)) = iter.next() {
            if !f(key, value) {
                break;
            }
        }
    }
}

impl Iterator<'_> {
    pub fn rewind(&self) {
        let mut index_iter = self.index_iter.write();
        index_iter.rewind();
    }

    pub fn seek(&self, key: &[u8]) {
        let mut index_iter = self.index_iter.write();
        index_iter.seek(key);
    }

    pub fn next(&self) -> Option<(Bytes, Bytes)> {
        let mut index_iter = self.index_iter.write();
        if let Some(item) = index_iter.next()
            && let Ok(value) = self.engine.get_value_by_position(item.1)
        {
            return Some((Bytes::from(item.0.to_vec()), value));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{options::Options, util::rand_kv::get_test_value};
    use anyhow::Result;
    use tempfile::TempDir;

    fn test_tmpdir() -> anyhow::Result<TempDir> {
        let parent = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        std::fs::create_dir_all(&parent)?;
        TempDir::new_in(parent).map_err(Into::into)
    }

    #[test]
    fn iterator_should_work() -> Result<()> {
        let tmp = test_tmpdir()?;
        let opts = Options {
            dir_path: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let engine = Engine::open(opts.clone())?;

        // list keys without data
        let keys = engine.list_keys();
        assert!(keys.is_empty());

        // 没有数据的情况
        let iter = engine.iter(IteratorOptions::default());
        iter.seek("aa".as_bytes());
        assert!(iter.next().is_none());

        // 有一条数据的情况
        engine.put(Bytes::from("aacc"), get_test_value(10))?;
        let iter = engine.iter(IteratorOptions::default());
        iter.seek("a".as_bytes());
        let ret = iter
            .next()
            .is_some_and(|item| item.0 == "aacc".as_bytes() && item.1 == get_test_value(10));
        assert!(ret);
        // rewind
        iter.rewind();
        let ret = iter
            .next()
            .is_some_and(|item| item.0 == "aacc".as_bytes() && item.1 == get_test_value(10));
        assert!(ret);

        // 多条数据的情况
        engine.put(Bytes::from("eecc"), get_test_value(10))?;
        engine.put(Bytes::from("bbac"), get_test_value(10))?;
        engine.put(Bytes::from("ccde"), get_test_value(10))?;

        let iter = engine.iter(IteratorOptions::default());
        iter.seek("a".as_bytes());

        let mut size = 0;
        while iter.next().is_some() {
            size += 1;
        }
        assert_eq!(size, 4);

        // 反向迭代
        let iter = engine.iter(IteratorOptions {
            reverse: true,
            ..Default::default()
        });
        iter.seek("z".as_bytes());

        let mut size = 0;
        while iter.next().is_some() {
            size += 1;
        }
        assert_eq!(size, 4);

        // list keys with data
        let keys = engine.list_keys();
        assert_eq!(
            vec![
                Bytes::from("aacc"),
                Bytes::from("bbac"),
                Bytes::from("ccde"),
                Bytes::from("eecc")
            ],
            keys
        );

        // fold
        engine.flod(|key, value| {
            println!("key: {:?}, value: {:?}", key, value);
            true
        });

        // 前缀
        let iter = engine.iter(IteratorOptions {
            prefix: "cc".as_bytes().to_vec(),
            ..Default::default()
        });
        let ret = iter
            .next()
            .is_some_and(|item| item.0 == "ccde".as_bytes() && item.1 == get_test_value(10));

        assert!(ret);

        Ok(())
    }
}
