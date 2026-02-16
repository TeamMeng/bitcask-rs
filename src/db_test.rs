#[cfg(test)]
mod tests {
    use crate::{
        db::Engine,
        errors::AppError,
        options::Options,
        util::rand_kv::{get_test_key, get_test_value},
    };
    use anyhow::Result;
    use bytes::Bytes;
    use std::{fs, path::PathBuf};

    #[test]
    fn engine_should_work() -> Result<()> {
        let opts = Options {
            dir_path: PathBuf::from("/tmp/bitcask-rs-test"),
            data_file_size: 64 * 1024 * 1024,
            ..Default::default()
        };
        let engine = Engine::open(opts.clone())?;
        // Get 一条不存在的数据
        let ret = engine
            .get(&get_test_key(11))
            .is_err_and(|e| e == AppError::KeyNotFound);
        assert!(ret);

        // Delete 一条不存在的数据
        let ret = engine.delete(&get_test_key(11)).is_ok();
        assert!(ret);

        // 正常 Put 一条数据
        engine.put(get_test_key(11), get_test_value(11))?;

        // 正常 Get 一条数据
        let ret = engine.get(&get_test_key(11))?;
        assert_eq!(ret, get_test_value(11));

        // Delete 一条数据
        engine.delete(&get_test_key(11))?;

        // Delete 再 Get
        let ret = engine
            .get(&get_test_key(11))
            .is_err_and(|e| e == AppError::KeyNotFound);
        assert!(ret);

        // Delete 再 Put
        engine.put(get_test_key(11), get_test_value(11))?;
        let ret = engine.get(&get_test_key(11))?;
        assert_eq!(ret, get_test_value(11));

        // key 为空
        let ret = engine
            .put(Bytes::new(), get_test_value(22))
            .is_err_and(|e| e == AppError::KeyIsEmpty);
        assert!(ret);

        // value 为空
        engine.put(get_test_key(33), Bytes::new())?;
        assert!(engine.get(&get_test_key(33))?.is_empty());

        // Delete 一个不存在的 key
        engine.delete(&Bytes::from("not-existed-key"))?;

        // Delete 一个空的 key
        let ret = engine
            .delete(&Bytes::new())
            .is_err_and(|e| e == AppError::KeyIsEmpty);
        assert!(ret);

        // Get 一个不存在的key
        let ret = engine
            .get(&Bytes::from("not-existed-key"))
            .is_err_and(|e| e == AppError::KeyNotFound);
        assert!(ret);

        // 重复 Put key 相同的数据
        engine.put(get_test_key(22), get_test_value(22))?;
        engine.put(get_test_key(22), Bytes::from("a new value"))?;
        let res = engine.get(&get_test_key(22))?;
        assert_eq!(res, Bytes::from("a new value"));

        // 写到数据文件
        for i in 500..=100000 {
            engine.put(get_test_key(i), get_test_value(i))?;
        }

        // 重启
        let engine = Engine::open(opts.clone())?;

        let ret = engine.get(&get_test_key(11))?;
        assert_eq!(ret, get_test_value(11));

        let res = engine.get(&get_test_key(22))?;
        assert_eq!(res, Bytes::from("a new value"));

        let res = engine.get(&get_test_key(660))?;
        assert_eq!(res, get_test_value(660));

        fs::remove_dir_all(opts.dir_path)?;
        Ok(())
    }
}
