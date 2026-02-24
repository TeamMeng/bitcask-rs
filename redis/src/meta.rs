use crate::types::RedisDataType;
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// 元数据
pub(crate) struct Metadate {
    /// 数据类型
    pub(crate) data_type: RedisDataType,
    /// 过期时间
    pub(crate) expire: u128,
    /// 版本号
    pub(crate) version: u128,
    /// 数据量
    pub(crate) size: u32,
    /// List 专用
    pub(crate) head: u64,
    /// List 专用
    pub(crate) tail: u64,
}

pub(crate) struct HashInternalKey {
    pub(crate) key: Vec<u8>,
    pub(crate) version: u128,
    pub(crate) field: Vec<u8>,
}

pub(crate) struct SetInternalKey {
    pub(crate) key: Vec<u8>,
    pub(crate) version: u128,
    pub(crate) member: Vec<u8>,
}

pub(crate) struct ListInternalKey {
    pub(crate) key: Vec<u8>,
    pub(crate) version: u128,
    pub(crate) index: u64,
}

impl Metadate {
    pub(crate) fn encode(&self) -> Bytes {
        let mut buf = BytesMut::new();

        // data type
        buf.put_u8(self.data_type as u8);
        // expire
        buf.put_u128(self.expire);
        // version
        buf.put_u128(self.version);
        // size
        buf.put_u32(self.size);

        // head and tail
        if self.data_type == RedisDataType::List {
            buf.put_u64(self.head);
            buf.put_u64(self.tail);
        }

        buf.into()
    }
}

pub(crate) fn decode_metadata(mut buf: Bytes) -> Metadate {
    // data type
    let data_type = RedisDataType::from(buf.get_u8());
    // expire
    let expire = buf.get_u128();
    // version
    let version = buf.get_u128();
    // size
    let size = buf.get_u32();

    let (mut head, mut tail) = (0, 0);
    if data_type == RedisDataType::List {
        head = buf.get_u64();
        tail = buf.get_u64();
    }

    Metadate {
        data_type,
        expire,
        version,
        size,
        head,
        tail,
    }
}

impl HashInternalKey {
    pub(crate) fn encode(&self) -> Bytes {
        let mut buf = BytesMut::new();

        buf.extend_from_slice(&self.key);
        buf.put_u128(self.version);
        buf.extend_from_slice(&self.field);

        buf.into()
    }
}

impl SetInternalKey {
    pub(crate) fn encode(&self) -> Bytes {
        let mut buf = BytesMut::new();

        buf.extend_from_slice(&self.key);
        buf.put_u128(self.version);
        buf.extend_from_slice(&self.member);
        buf.put_u32(self.member.len() as u32);

        buf.into()
    }
}

impl ListInternalKey {
    pub(crate) fn encode(&self) -> Bytes {
        let mut buf = BytesMut::new();

        buf.extend_from_slice(&self.key);
        buf.put_u128(self.version);
        buf.put_u64(self.index);

        buf.into()
    }
}
