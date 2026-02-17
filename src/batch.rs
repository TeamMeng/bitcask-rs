use crate::{
    data::log_record::{LogRecord, LogRecordType},
    db::Engine,
    errors::AppError,
    options::WriteBatchOptions,
};
use bytes::{BufMut, Bytes, BytesMut};
use parking_lot::Mutex;
use prost::{decode_length_delimiter, encode_length_delimiter};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};

const TXN_FIN_KEY: &[u8] = "txn-fin".as_bytes();
pub(crate) const NON_TRANSACTION_SEQ_NO: usize = 0;

/// 批量写操作，保证原子性
pub struct WriteBatch<'a> {
    /// 暂存用户写入的数据
    pending_write: Arc<Mutex<HashMap<Vec<u8>, LogRecord>>>,
    engine: &'a Engine,
    options: WriteBatchOptions,
}

impl Engine {
    pub fn new_write_batch(&self, options: WriteBatchOptions) -> Result<WriteBatch<'_>, AppError> {
        Ok(WriteBatch {
            pending_write: Arc::new(Mutex::new(HashMap::new())),
            engine: self,
            options,
        })
    }
}

impl WriteBatch<'_> {
    /// 批量操作写数据
    pub fn put(&self, key: Bytes, value: Bytes) -> Result<(), AppError> {
        if key.is_empty() {
            return Err(AppError::KeyIsEmpty);
        }

        let log_record = LogRecord::new(key.to_vec(), value.to_vec(), LogRecordType::Normal);
        let mut pending_writes = self.pending_write.lock();
        pending_writes.insert(key.to_vec(), log_record);

        Ok(())
    }

    /// 批量删除操作
    pub fn delete(&self, key: Bytes) -> Result<(), AppError> {
        if key.is_empty() {
            return Err(AppError::KeyIsEmpty);
        }
        let mut pending_writes = self.pending_write.lock();
        // 如果数据不存在则直接返回
        let index_pos = self.engine.get(&key);
        if index_pos.is_err() && pending_writes.contains_key(&key.to_vec()) {
            pending_writes.remove(&key.to_vec());
            return Ok(());
        }

        let log_record = LogRecord::new(key.to_vec(), Default::default(), LogRecordType::Delete);
        pending_writes.insert(key.to_vec(), log_record);

        Ok(())
    }

    /// 提交数据，将数据写到文件中，并更新内存索引
    pub fn commit(&self) -> Result<(), AppError> {
        let mut pending_writes = self.pending_write.lock();
        if pending_writes.is_empty() {
            return Ok(());
        }

        if pending_writes.len() > self.options.max_batch_num {
            return Err(AppError::ExceedMaxBatchNum);
        }

        // 加锁保证事务提交串行化
        let _lock = self.engine.batch_commit_lock.lock();

        // 获取全局事务序列号
        let seq_no = self.engine.seq_no.fetch_add(1, Ordering::SeqCst);

        let mut positions = HashMap::new();
        // 开始写数据到数据文件中
        for (_, item) in pending_writes.iter() {
            let log_record = LogRecord::new(
                log_record_key_with_seq(item.key.clone(), seq_no)?,
                item.value.clone(),
                item.rec_type,
            );

            let pos = self.engine.append_log_record(&log_record)?;
            positions.insert(item.key.clone(), pos);
        }

        // 写最后一条标识事务完成的数据
        let finish_record = LogRecord::new(
            log_record_key_with_seq(TXN_FIN_KEY.to_vec(), seq_no)?,
            Default::default(),
            LogRecordType::TxnFinished,
        );
        self.engine.append_log_record(&finish_record)?;

        // 如果配置了持久化，则sync
        if self.options.sync_writes {
            self.engine.sync()?;
        }

        // 数据全部写完之后更新内存索引
        for (_, item) in pending_writes.iter() {
            if let Some(record_pos) = positions.get(&item.key) {
                if item.rec_type == LogRecordType::Normal {
                    self.engine.index.put(item.key.clone(), *record_pos);
                }
                if item.rec_type == LogRecordType::Delete {
                    self.engine.index.delete(&item.key);
                }
            }
        }

        // 清空暂存数据
        pending_writes.clear();
        Ok(())
    }
}

/// 编码 seq no 和 key
pub(crate) fn log_record_key_with_seq(key: Vec<u8>, seq_no: usize) -> Result<Vec<u8>, AppError> {
    let mut enc_key = BytesMut::new();
    if encode_length_delimiter(seq_no, &mut enc_key).is_err() {
        return Err(AppError::EncodeError);
    }
    enc_key.extend_from_slice(&key.to_vec());
    Ok(enc_key.to_vec())
}

/// 解析 LogRecord 的 key，拿到实际的 key 和seq no
pub(crate) fn parse_log_record_key(key: Vec<u8>) -> Result<(Vec<u8>, usize), AppError> {
    let mut buf = BytesMut::new();
    buf.put_slice(&key);
    let Ok(seq_no) = decode_length_delimiter(&mut buf) else {
        return Err(AppError::DecodeError);
    };

    Ok((buf.to_vec(), seq_no))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::Options;
    use crate::util::rand_kv::{get_test_key, get_test_value};
    use anyhow::Result;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn write_batch_should_work() -> Result<()> {
        let opts = Options {
            dir_path: PathBuf::from("/tmp/bitcask-rs-write-batch-test"),
            data_file_size: 64 * 1024 * 1024,
            ..Default::default()
        };
        let engine = Engine::open(opts.clone())?;

        let wb = engine.new_write_batch(WriteBatchOptions::default())?;
        // 写数据之后未提交
        wb.put(get_test_key(1), get_test_value(10))?;
        wb.put(get_test_key(2), get_test_value(10))?;

        let ret = engine
            .get(&get_test_key(1))
            .is_err_and(|e| e == AppError::KeyNotFound);
        assert!(ret);

        // 事务提交之后进行查询
        wb.commit()?;

        let ret = engine
            .get(&get_test_key(1))
            .is_ok_and(|value| value == get_test_value(10));
        assert!(ret);

        // 验证事务序列号
        let seq_no = wb.engine.seq_no.load(Ordering::SeqCst);
        assert_eq!(seq_no, 2);

        wb.put(get_test_key(1), get_test_value(10))?;
        wb.commit()?;

        // 重启之后进行检验
        engine.close()?;

        let keys = engine.list_keys();
        assert_eq!(
            vec![
                Bytes::from("bitcasl-rs-key-000000001"),
                Bytes::from("bitcasl-rs-key-000000002")
            ],
            keys
        );
        let seq_no = wb.engine.seq_no.load(Ordering::SeqCst);
        assert_eq!(seq_no, 3);

        fs::remove_dir_all(opts.dir_path)?;
        Ok(())
    }
}
