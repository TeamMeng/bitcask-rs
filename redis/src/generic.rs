use crate::types::{RedisDataStructure, RedisDataType};
use bitcask_rs::errors::AppError;
use bytes::{Buf, Bytes};

impl RedisDataStructure {
    pub fn del(&self, key: &str) -> Result<(), AppError> {
        self.engine.delete(&Bytes::copy_from_slice(key.as_bytes()))
    }

    pub fn key_type(&self, key: &str) -> Result<RedisDataType, AppError> {
        let mut buf = self.engine.get(&Bytes::copy_from_slice(key.as_bytes()))?;
        Ok(RedisDataType::from(buf.get_u8()))
    }
}
