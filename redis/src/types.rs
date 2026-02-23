use bitcask_rs::{db::Engine, errors::AppError, options::Options};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, PartialEq, Eq)]
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

    /// String 数据结构
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
    use std::{fs, path::PathBuf};

    use super::*;
    use anyhow::Result;

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
}
