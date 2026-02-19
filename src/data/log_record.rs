use crate::errors::AppError;
use bytes::{BufMut, BytesMut};
use crc32fast::Hasher;
use log::warn;
use prost::encoding::{decode_varint, encode_varint};
use prost::{encode_length_delimiter, length_delimiter_len};
use std::mem;

/// 写入到数据文件的记录，追加写入的数据类似日志的格式
pub struct LogRecord {
    pub(crate) key: Vec<u8>,
    pub(crate) value: Vec<u8>,
    pub(crate) rec_type: LogRecordType,
}

/// 数据位置索引信息，描述数据存储到了哪个位置
#[derive(Debug, Clone, Copy)]
pub struct LogRecordPos {
    /// 文件 id
    pub(crate) file_id: u32,
    /// 偏移量
    pub(crate) offset: u64,
}

/// 数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogRecordType {
    /// 正常 Put 的数据
    Normal = 1,
    /// 被删除的数据标记，墓碑值
    Delete = 2,

    // 事务完成的标识
    TxnFinished = 3,
}

/// 从数据文件中读取的 log_record 信息，包含其 size
pub struct ReadLogRecord {
    pub(crate) record: LogRecord,
    pub(crate) size: usize,
}

/// 暂存事务数据信息
pub struct TransactionRecord {
    pub(crate) record: LogRecord,
    pub(crate) pos: LogRecordPos,
}

/// 获取 LogRecord header 部分的最大长度
pub fn max_log_record_header_size() -> usize {
    mem::size_of::<u8>() + length_delimiter_len(u32::MAX as usize) * 2
}

/// 解码 LogRecordPos
pub fn decode_log_record_pos(pos: Vec<u8>) -> LogRecordPos {
    let mut buf = BytesMut::new();
    buf.put_slice(&pos);

    let file_id = match decode_varint(&mut buf) {
        Ok(fid) => fid,
        Err(e) => panic!("decode log record pos err: {}", e),
    };
    let offset = match decode_varint(&mut buf) {
        Ok(offset) => offset,
        Err(e) => panic!("decode log record pos err: {}", e),
    };

    LogRecordPos::new(file_id as _, offset)
}

impl LogRecord {
    pub fn new(key: Vec<u8>, value: Vec<u8>, rec_type: LogRecordType) -> Self {
        Self {
            key,
            value,
            rec_type,
        }
    }

    /// encode log record，返回字节数组及长度
    // +------------+-------------+------------+-----------+-----------+---------+
    // |  crc检验值 |   type类型  |  key size  | value size|    key    |  value  |
    // +------------+-------------+------------+-----------+-----------+---------+
    //     4 字节        1字节      变长（最大5）变长（最大5）  变长      变长
    pub fn encode(&self) -> Result<Vec<u8>, AppError> {
        Ok(self.encode_and_get_crc()?.0)
    }

    pub fn get_crc(&self) -> Result<u32, AppError> {
        Ok(self.encode_and_get_crc()?.1)
    }

    fn encode_and_get_crc(&self) -> Result<(Vec<u8>, u32), AppError> {
        // 初始化字节数组，存储编码数据
        let mut buf = BytesMut::new();
        buf.reserve(self.encoded_length());

        // 第一个字节 type 类型
        buf.put_u8(self.rec_type as u8);

        // 再存储 key 和 value 的长度
        if let Err(e) = encode_length_delimiter(self.key.len(), &mut buf) {
            warn!("encode error: {}", e);
            return Err(AppError::EncodeError);
        }

        if let Err(e) = encode_length_delimiter(self.value.len(), &mut buf) {
            warn!("encode error: {}", e);
            return Err(AppError::EncodeError);
        }

        // 存储 key 和 value
        buf.extend_from_slice(&self.key);
        buf.extend_from_slice(&self.value);

        // 计算并存储 crc 检验值
        let mut hashder = Hasher::new();
        hashder.update(&buf);
        let crc = hashder.finalize();
        buf.put_u32(crc);

        Ok((buf.to_vec(), crc))
    }

    fn encoded_length(&self) -> usize {
        mem::size_of::<u8>()
            + length_delimiter_len(self.key.len())
            + length_delimiter_len(self.value.len())
            + self.key.len()
            + self.value.len()
            + 4
    }
}

impl LogRecordPos {
    pub fn new(file_id: u32, offset: u64) -> Self {
        Self { file_id, offset }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = BytesMut::new();
        encode_varint(self.file_id as u64, &mut buf);
        encode_varint(self.offset, &mut buf);
        buf.to_vec()
    }
}

impl LogRecordType {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => LogRecordType::Normal,
            2 => LogRecordType::Delete,
            3 => LogRecordType::TxnFinished,
            _ => panic!("invalid log record type"),
        }
    }
}

impl ReadLogRecord {
    pub fn new(record: LogRecord, size: usize) -> Self {
        Self { record, size }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn log_record_encode_should_work() -> Result<()> {
        // 正常的一条 LogRecord 编码
        let mut log_record = LogRecord::new(
            "name".as_bytes().to_vec(),
            "bitcask-rs".as_bytes().to_vec(),
            LogRecordType::Normal,
        );

        let enc = log_record.encode()?;
        assert_eq!(
            vec![
                1, 4, 10, 110, 97, 109, 101, 98, 105, 116, 99, 97, 115, 107, 45, 114, 115, 60, 209,
                119, 130
            ],
            enc
        );

        let crc = log_record.get_crc()?;
        assert_eq!(1020360578, crc);

        // LogRecord 的 value 为空
        log_record.value = Default::default();
        let enc = log_record.encode()?;
        assert_eq!(vec![1, 4, 0, 110, 97, 109, 101, 223, 237, 55, 198], enc);

        let crc = log_record.get_crc()?;
        assert_eq!(3756865478, crc);

        // 类型为 Deleted 的情况
        log_record.value = "bitcask-rs".as_bytes().to_vec();
        log_record.rec_type = LogRecordType::Delete;

        let enc = log_record.encode()?;
        assert_eq!(
            vec![
                2, 4, 10, 110, 97, 109, 101, 98, 105, 116, 99, 97, 115, 107, 45, 114, 115, 111, 75,
                44, 6
            ],
            enc
        );

        let crc = log_record.get_crc()?;
        assert_eq!(1867197446, crc);
        Ok(())
    }
}
