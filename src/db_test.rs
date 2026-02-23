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
    use tempfile::TempDir;

    fn test_tmpdir() -> Result<TempDir> {
        let parent = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        std::fs::create_dir_all(&parent)?;
        TempDir::new_in(parent).map_err(Into::into)
    }

    #[test]
    fn engine_should_work() -> Result<()> {
        let tmp = test_tmpdir()?;
        let opts = Options {
            dir_path: tmp.path().to_path_buf(),
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
        std::mem::drop(engine);
        let engine = Engine::open(opts.clone())?;

        let ret = engine.get(&get_test_key(11))?;
        assert_eq!(ret, get_test_value(11));

        let res = engine.get(&get_test_key(22))?;
        assert_eq!(res, Bytes::from("a new value"));

        let res = engine.get(&get_test_key(660))?;
        assert_eq!(res, get_test_value(660));

        Ok(())
    }

    #[test]
    fn engine_filelock_should_work() -> Result<()> {
        let tmp = test_tmpdir()?;
        let opts = Options {
            dir_path: tmp.path().to_path_buf(),
            data_file_size: 64 * 1024 * 1024,
            ..Default::default()
        };
        let engine = Engine::open(opts.clone())?;

        let ret = Engine::open(opts.clone()).is_err_and(|e| e == AppError::DatabaseIsUsing);
        assert!(ret);

        engine.close()?;
        let _engine = Engine::open(opts.clone())?;

        Ok(())
    }

    #[test]
    fn engine_stat_should_work() -> Result<()> {
        let tmp = test_tmpdir()?;
        let opts = Options {
            dir_path: tmp.path().to_path_buf(),
            data_file_size: 64 * 1024 * 1024,
            ..Default::default()
        };
        let engine = Engine::open(opts.clone())?;

        for i in 0..=100000 {
            engine.put(get_test_key(i), get_test_value(i))?;
        }

        for i in 0..=1000 {
            engine.put(get_test_key(i), get_test_value(i))?;
        }

        for i in 2000..=5000 {
            engine.delete(&get_test_key(i))?;
        }

        let stat = engine.stat()?;
        assert_eq!(stat.key_num, 97000);
        assert_eq!(stat.data_file_num, 1);
        assert_eq!(stat.reclaim_size, 325147);

        Ok(())
    }

    #[test]
    fn engine_merge_should_work() -> Result<()> {
        // 没有任何数据的情况下进行 merge
        let opts = Options {
            dir_path: PathBuf::from("/tmp/bitcask-rs-merge"),
            data_file_size: 32 * 1024 * 1024,
            data_file_merge_ratio: 0 as f32,
            ..Default::default()
        };
        let engine = Engine::open(opts.clone())?;

        let ret = engine.merge();
        assert!(ret.is_ok());

        // 全部都是有效数据
        for i in 0..50000 {
            engine.put(get_test_key(i), get_test_value(i))?;
        }

        engine.merge()?;
        // 重启
        drop(engine);
        let engine = Engine::open(opts.clone())?;
        let keys = engine.list_keys();
        assert_eq!(keys.len(), 50000);

        for i in 0..50000 {
            let ret = engine.get(&get_test_key(i))?;
            assert_eq!(ret, get_test_value(i));
        }

        fs::remove_dir_all(opts.dir_path)?;
        Ok(())
    }

    #[test]
    fn engine_backup_should_work() -> Result<()> {
        let opts = Options {
            dir_path: PathBuf::from("/tmp/bitcask-rs-backup"),
            ..Default::default()
        };
        let engine = Engine::open(opts.clone())?;

        for i in 0..=10000 {
            engine.put(get_test_key(i), get_test_value(i))?;
        }

        let back_up = PathBuf::from("/tmp/bitcask-rs-backup-test");
        engine.backup(back_up.clone())?;

        let opts = Options {
            dir_path: back_up.clone(),
            ..Default::default()
        };

        Engine::open(opts.clone())?;

        fs::remove_dir_all(PathBuf::from("/tmp/bitcask-rs-backup"))?;
        fs::remove_dir_all(back_up)?;
        Ok(())
    }
}
