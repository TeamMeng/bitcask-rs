use crate::meta::{
    HashInternalKey, ListInternalKey, Metadate, SetInternalKey, ZSetInternalKey, decode_metadata,
};
use bitcask_rs::{
    db::Engine,
    errors::AppError,
    options::{Options, WriteBatchOptions},
};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const INITIAL_LIST_MARK: u64 = u64::MAX / 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedisDataType {
    String,
    Hash,
    Set,
    List,
    ZSet,
}

/// Redis 数据结构服务
pub struct RedisDataStructure {
    pub(crate) engine: Engine,
}

impl RedisDataStructure {
    pub fn new(options: Options) -> Result<Self, AppError> {
        Ok(Self {
            engine: Engine::open(options)?,
        })
    }
    /// ================= String 数据结构 ==================
    pub fn set(&self, key: &str, value: &str, ttl: Duration) -> Result<(), AppError> {
        if value.is_empty() {
            return Ok(());
        }

        // 编码 value： type + expire + payload
        let mut buf = BytesMut::new();
        buf.put_u8(RedisDataType::String as u8);
        let mut exprie = 0;
        if ttl != Duration::ZERO
            && let Some(v) = SystemTime::now().checked_add(ttl)
        {
            exprie = v.duration_since(UNIX_EPOCH).unwrap().as_nanos();
        }

        buf.put_u128(exprie);

        buf.extend_from_slice(value.as_bytes());

        // 调用存储引擎的接口写入
        self.engine
            .put(Bytes::copy_from_slice(key.as_bytes()), buf.into())
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, AppError> {
        let mut buf = self.engine.get(&Bytes::copy_from_slice(key.as_bytes()))?;
        let key_type = RedisDataType::from(buf.get_u8());
        if key_type != RedisDataType::String {
            return Err(AppError::WrongTypeOperation);
        }

        // 判断过去时间
        let exprie = buf.get_u128();
        if exprie > 0
            && exprie
                <= SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
        {
            return Ok(None);
        }

        let value = buf.to_vec();
        Ok(Some(String::from_utf8(value).unwrap()))
    }

    /// ================= Hash 数据结构 ==================
    pub fn hset(&self, key: &str, field: &str, value: &str) -> Result<bool, AppError> {
        // 查询元数据
        let mut meta = self.find_metadata(key, RedisDataType::Hash)?;

        // 初始化数据部分的 key
        let hk = HashInternalKey {
            key: key.as_bytes().to_vec(),
            version: meta.version,
            field: field.as_bytes().to_vec(),
        };

        let mut exist = true;
        // 查询是否存在
        if let Err(e) = self.engine.get(&hk.encode())
            && e == AppError::KeyNotFound
        {
            exist = false;
        }

        let wb = self.engine.new_write_batch(WriteBatchOptions::default())?;
        if !exist {
            meta.size += 1;
            wb.put(Bytes::copy_from_slice(key.as_bytes()), meta.encode())?;
        }

        wb.put(hk.encode(), Bytes::copy_from_slice(value.as_bytes()))?;
        wb.commit()?;

        Ok(!exist)
    }

    pub fn hget(&self, key: &str, field: &str) -> Result<Option<String>, AppError> {
        let meta = self.find_metadata(key, RedisDataType::Hash)?;
        if meta.size == 0 {
            return Ok(None);
        }

        // 初始化数据部分的 key
        let hk = HashInternalKey {
            key: key.as_bytes().to_vec(),
            version: meta.version,
            field: field.as_bytes().to_vec(),
        };

        let value = self.engine.get(&hk.encode())?;

        Ok(Some(String::from_utf8(value.to_vec()).unwrap()))
    }

    pub fn hdel(&self, key: &str, field: &str) -> Result<bool, AppError> {
        let mut meta = self.find_metadata(key, RedisDataType::Hash)?;
        if meta.size == 0 {
            return Ok(false);
        }

        // 初始化数据部分的 key
        let hk = HashInternalKey {
            key: key.as_bytes().to_vec(),
            version: meta.version,
            field: field.as_bytes().to_vec(),
        };

        let mut exist = true;
        // 查询是否存在
        if let Err(e) = self.engine.get(&hk.encode())
            && e == AppError::KeyNotFound
        {
            exist = false;
        }

        if exist {
            let wb = self.engine.new_write_batch(WriteBatchOptions::default())?;
            meta.size -= 1;
            wb.put(Bytes::copy_from_slice(key.as_bytes()), meta.encode())?;
            wb.delete(hk.encode())?;
            wb.commit()?;
        }

        Ok(exist)
    }

    fn find_metadata(&self, key: &str, data_type: RedisDataType) -> Result<Metadate, AppError> {
        let mut exist = true;
        let mut meta = None;
        match self.engine.get(&Bytes::copy_from_slice(key.as_bytes())) {
            Ok(meta_buf) => {
                // 判断类型是否匹配
                let typ = &meta_buf[0..1];
                if data_type != RedisDataType::from(typ[0]) {
                    return Err(AppError::WrongTypeOperation);
                }
                meta = Some(decode_metadata(meta_buf));
                // 判断是否过期
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let expire = meta.as_ref().unwrap().expire;
                if expire != 0 && expire <= now {
                    exist = false;
                }
            }
            Err(e) => {
                if e != AppError::KeyNotFound {
                    return Err(e);
                }
                exist = false;
            }
        }

        if !exist {
            let mut metadata = Metadate {
                data_type,
                expire: 0,
                version: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                size: 0,
                head: 0,
                tail: 0,
            };
            if data_type == RedisDataType::List {
                metadata.head = INITIAL_LIST_MARK;
                metadata.tail = INITIAL_LIST_MARK;
            }
            meta = Some(metadata);
        }

        Ok(meta.unwrap())
    }

    /// ================= Set 数据结构 ==================
    pub fn sadd(&self, key: &str, members: &str) -> Result<bool, AppError> {
        // 查找元数据
        let mut meta = self.find_metadata(key, RedisDataType::Set)?;

        // 构造数据部分的 key
        let sk = SetInternalKey {
            key: key.as_bytes().to_vec(),
            version: meta.version,
            member: members.as_bytes().to_vec(),
        };

        if let Err(e) = self.engine.get(&sk.encode()) {
            // 不存在则更新
            if e == AppError::KeyNotFound {
                // 更新元数据
                let wb = self.engine.new_write_batch(WriteBatchOptions::default())?;
                meta.size += 1;
                wb.put(Bytes::copy_from_slice(key.as_bytes()), meta.encode())?;
                // 更新数据部分
                wb.put(sk.encode(), Bytes::new())?;
                wb.commit()?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn s_is_member(&self, key: &str, member: &str) -> Result<bool, AppError> {
        let meta = self.find_metadata(key, RedisDataType::Set)?;
        if meta.size == 0 {
            return Ok(false);
        }

        // 构造数据部分的 key
        let sk = SetInternalKey {
            key: key.as_bytes().to_vec(),
            version: meta.version,
            member: member.as_bytes().to_vec(),
        };

        match self.engine.get(&sk.encode()) {
            Ok(_) => Ok(true),
            Err(e) => {
                if e != AppError::KeyNotFound {
                    return Err(e);
                }
                Ok(false)
            }
        }
    }

    pub fn srem(&self, key: &str, member: &str) -> Result<bool, AppError> {
        let mut meta = self.find_metadata(key, RedisDataType::Set)?;
        if meta.size == 0 {
            return Ok(false);
        }

        // 构造数据部分的 key
        let sk = SetInternalKey {
            key: key.as_bytes().to_vec(),
            version: meta.version,
            member: member.as_bytes().to_vec(),
        };

        if self.engine.get(&sk.encode()).is_ok() {
            // 更新元数据
            meta.size -= 1;
            let wb = self.engine.new_write_batch(WriteBatchOptions::default())?;
            wb.put(Bytes::copy_from_slice(key.as_bytes()), meta.encode())?;
            // 写数据部分
            wb.delete(sk.encode())?;
            wb.commit()?;
            return Ok(true);
        }

        Ok(false)
    }

    /// ================= List 数据结构 ==================
    pub fn lpush(&self, key: &str, element: &str) -> Result<u32, AppError> {
        self.push_inner(key, element, true)
    }

    pub fn rpush(&self, key: &str, element: &str) -> Result<u32, AppError> {
        self.push_inner(key, element, false)
    }

    pub fn lpop(&self, key: &str) -> Result<Option<String>, AppError> {
        self.pop_inner(key, true)
    }

    pub fn rpop(&self, key: &str) -> Result<Option<String>, AppError> {
        self.pop_inner(key, false)
    }

    fn push_inner(&self, key: &str, element: &str, is_left: bool) -> Result<u32, AppError> {
        // 查找元数据
        let mut meta = self.find_metadata(key, RedisDataType::List)?;

        // 构造数据部分的 key
        let lk = ListInternalKey {
            key: key.as_bytes().to_vec(),
            version: meta.version,
            index: match is_left {
                true => meta.head - 1,
                false => meta.tail,
            },
        };

        // 更新数据和元数据
        let wb = self.engine.new_write_batch(WriteBatchOptions::default())?;
        meta.size += 1;
        if is_left {
            meta.head -= 1;
        } else {
            meta.tail += 1;
        }
        wb.put(Bytes::copy_from_slice(key.as_bytes()), meta.encode())?;
        wb.put(lk.encode(), Bytes::copy_from_slice(element.as_bytes()))?;
        wb.commit()?;

        Ok(meta.size)
    }

    fn pop_inner(&self, key: &str, is_left: bool) -> Result<Option<String>, AppError> {
        // 查询元数据部分
        let mut meta = self.find_metadata(key, RedisDataType::List)?;
        if meta.size == 0 {
            return Ok(None);
        }

        // 构造数据部分的 key
        let lk = ListInternalKey {
            key: key.as_bytes().to_vec(),
            version: meta.version,
            index: match is_left {
                true => meta.head,
                false => meta.tail - 1,
            },
        };

        let element = self.engine.get(&lk.encode())?;

        // 更新元数据
        meta.size -= 1;
        if is_left {
            meta.head += 1;
        } else {
            meta.tail -= 1;
        }

        self.engine
            .put(Bytes::copy_from_slice(key.as_bytes()), meta.encode())?;

        Ok(Some(String::from_utf8(element.to_vec()).unwrap()))
    }

    /// ================= ZSet 数据结构 ==================
    pub fn zadd(&self, key: &str, score: f64, member: &str) -> Result<bool, AppError> {
        let mut meta = self.find_metadata(key, RedisDataType::ZSet)?;

        // 构造数据部分的 key
        let zk = ZSetInternalKey {
            key: key.as_bytes().to_vec(),
            version: meta.version,
            score,
            member: member.as_bytes().to_vec(),
        };

        let mut exist = true;
        let mut old_score = 0.0;
        match self.engine.get(&zk.encode_member()) {
            Ok(val) => {
                let val = String::from_utf8(val.to_vec()).unwrap();
                old_score = val.parse().unwrap();
                if old_score == score {
                    return Ok(false);
                }
            }
            Err(e) => {
                if e != AppError::KeyNotFound {
                    return Err(e);
                }
                exist = false;
            }
        }

        let wb = self.engine.new_write_batch(WriteBatchOptions::default())?;
        // 更新元数据部分
        if !exist {
            meta.size += 1;
            wb.put(Bytes::copy_from_slice(key.as_bytes()), meta.encode())?;
        }

        if exist {
            let old_zk = ZSetInternalKey {
                key: key.as_bytes().to_vec(),
                version: meta.version,
                score: old_score,
                member: member.as_bytes().to_vec(),
            };
            wb.delete(old_zk.encode_score())?;
        }

        wb.put(zk.encode_member(), Bytes::from(score.to_string()))?;
        wb.put(zk.encode_score(), Bytes::new())?;
        wb.commit()?;

        Ok(!exist)
    }

    pub fn zscore(&self, key: &str, member: &str) -> Result<f64, AppError> {
        let meta = self.find_metadata(key, RedisDataType::ZSet)?;
        if meta.size == 0 {
            return Ok(-1_f64);
        }

        // 构造数据部分的 key
        let zk = ZSetInternalKey {
            key: key.as_bytes().to_vec(),
            version: meta.version,
            score: 0.0,
            member: member.as_bytes().to_vec(),
        };

        let score = self.engine.get(&zk.encode_member())?;
        Ok(String::from_utf8(score.to_vec()).unwrap().parse().unwrap())
    }
}

impl From<u8> for RedisDataType {
    fn from(value: u8) -> Self {
        match value {
            0 => RedisDataType::String,
            1 => RedisDataType::Hash,
            2 => RedisDataType::Set,
            3 => RedisDataType::List,
            4 => RedisDataType::ZSet,
            _ => panic!("invalid redis data type"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::{fs, path::PathBuf};

    #[test]
    fn redis_should_work() -> Result<()> {
        let opts = Options {
            dir_path: PathBuf::from("/tmp/bitcask-rs-redis"),
            ..Default::default()
        };
        let rds = RedisDataStructure::new(opts.clone())?;

        rds.set("key1", "val1", Duration::ZERO)?;
        rds.set("key2", "val2", Duration::from_secs(5))?;

        let v1 = rds.get("key1")?.is_some_and(|v| v == "val1");
        assert!(v1);
        let v2 = rds.get("key2")?.is_some_and(|v| v == "val2");
        assert!(v2);

        let key_type = rds.key_type("key1")?;
        assert_eq!(key_type, RedisDataType::String);

        rds.del("key1")?;

        let v1 = rds.get("key1").is_err_and(|v| v == AppError::KeyNotFound);
        assert!(v1);

        fs::remove_dir_all(opts.dir_path)?;
        Ok(())
    }

    #[test]
    fn redis_h_should_work() -> Result<()> {
        let opts = Options {
            dir_path: PathBuf::from("/tmp/bitcask-rs-redis-h"),
            ..Default::default()
        };
        let rds = RedisDataStructure::new(opts.clone())?;

        let ret = rds.hdel("myhash", "field").is_ok_and(|v| !v);
        assert!(ret);

        rds.hset("myhash", "field1", "value-1")?;
        rds.hset("myhash", "field1", "value-2")?;
        rds.hset("myhash", "field2", "value-3")?;

        let ret = rds
            .hget("myhash", "field1")?
            .is_some_and(|v| v == "value-2");
        assert!(ret);

        let ret = rds
            .hget("myhash", "field2")?
            .is_some_and(|v| v == "value-3");
        assert!(ret);

        let ret = rds
            .hget("myhash", "field-not-exist")
            .is_err_and(|e| e == AppError::KeyNotFound);
        assert!(ret);

        rds.del("myhash")?;

        let ret = rds
            .hget("myhash", "field-not-exist")
            .is_ok_and(|v| v.is_none());
        assert!(ret);

        fs::remove_dir_all(opts.dir_path)?;
        Ok(())
    }

    #[test]
    fn redis_s_should_work() -> Result<()> {
        let opts = Options {
            dir_path: PathBuf::from("/tmp/bitcask-rs-redis-s"),
            ..Default::default()
        };
        let rds = RedisDataStructure::new(opts.clone())?;

        let ret = rds.srem("myset", "val-1").is_ok_and(|v| !v);
        assert!(ret);

        rds.sadd("myset", "val-1")?;
        let ret = rds.sadd("myset", "val-1").is_ok_and(|v| !v);
        assert!(ret);
        rds.sadd("myset", "val-2")?;

        let ret = rds.s_is_member("myset-1", "val-1").is_ok_and(|v| !v);
        assert!(ret);
        let ret = rds.s_is_member("myset", "val-1")?;
        assert!(ret);
        let ret = rds.s_is_member("myset", "val-2")?;
        assert!(ret);
        let ret = rds.s_is_member("myset", "val-not-exist").is_ok_and(|v| !v);
        assert!(ret);

        rds.srem("myset", "val-1")?;
        let ret = rds.s_is_member("myset", "val-1").is_ok_and(|v| !v);
        assert!(ret);

        fs::remove_dir_all(opts.dir_path)?;
        Ok(())
    }

    #[test]
    fn redis_l_should_work() -> Result<()> {
        let opts = Options {
            dir_path: PathBuf::from("/tmp/bitcask-rs-redis-l"),
            ..Default::default()
        };
        let rds = RedisDataStructure::new(opts.clone())?;

        // empty pop
        let ret = rds.lpop("myList").is_ok_and(|v| v.is_none());
        assert!(ret);
        let ret = rds.rpop("myList").is_ok_and(|v| v.is_none());
        assert!(ret);

        rds.lpush("myList", "aa")?;
        rds.rpush("myList", "bb")?;
        rds.rpush("myList", "cc")?;

        let ret = rds
            .lpop("myList")
            .is_ok_and(|v| v.is_some_and(|s| s == "aa"));
        assert!(ret);

        let ret = rds
            .rpop("myList")
            .is_ok_and(|v| v.is_some_and(|s| s == "cc"));
        assert!(ret);

        fs::remove_dir_all(opts.dir_path)?;
        Ok(())
    }

    #[test]
    fn redis_z_should_work() -> Result<()> {
        let opts = Options {
            dir_path: PathBuf::from("/tmp/bitcask-rs-redis-z"),
            ..Default::default()
        };
        let rds = RedisDataStructure::new(opts.clone())?;

        rds.zadd("myzset", 12.11, "val-1")?;
        rds.zadd("myzset", 33.11, "val-1")?;
        rds.zadd("myzset", 33.99, "val-2")?;

        let ret = rds.zscore("myzset", "val-1").is_ok_and(|v| v == 33.11);
        assert!(ret);

        let ret = rds.zscore("myzset", "val-2").is_ok_and(|v| v == 33.99);
        assert!(ret);

        fs::remove_dir_all(opts.dir_path)?;
        Ok(())
    }
}
