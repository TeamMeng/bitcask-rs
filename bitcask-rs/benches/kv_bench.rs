use bitcask_rs::{db::Engine, options::Options};
use bytes::Bytes;
use criterion::{Criterion, criterion_group, criterion_main};
use rand::seq::{IndexedRandom, SliceRandom};
use std::path::PathBuf;

pub fn get_test_key(i: u32) -> Bytes {
    Bytes::from(format!("bitcasl-rs-key-{:1009}", i))
}

pub fn get_test_value(i: u32) -> Bytes {
    Bytes::from(format!("bitcasl-rs-value-{:1009}", i))
}

fn benchmark_put(c: &mut Criterion) {
    // 打开存储引擎
    let opts = Options {
        dir_path: PathBuf::from("/tmp/bitcask-rs-bench-put"),
        ..Default::default()
    };

    let mut rng = rand::rng();
    let mut nums: Vec<u32> = (0..u32::MAX).collect();

    let engine = Engine::open(opts).unwrap();
    c.bench_function("bitcask-put-bench", |b| {
        b.iter(|| {
            nums.shuffle(&mut rng);
            let num = nums.choose(&mut rng);
            let n = match num {
                Some(num) => *num,
                None => 0,
            };
            let ret = engine.put(get_test_key(n), get_test_value(n));
            assert!(ret.is_ok());
        });
    });
}

fn benchmark_get(c: &mut Criterion) {
    // 打开存储引擎
    let opts = Options {
        dir_path: PathBuf::from("/tmp/bitcask-rs-bench-get"),
        ..Default::default()
    };

    let engine = Engine::open(opts).unwrap();

    for i in 0..100000 {
        let ret = engine.put(get_test_key(i), get_test_value(i));
        assert!(ret.is_ok());
    }

    let mut rng = rand::rng();
    let mut nums: Vec<u32> = (0..u32::MAX).collect();

    c.bench_function("bitcask-get-bench", |b| {
        b.iter(|| {
            nums.shuffle(&mut rng);
            let num = nums.choose(&mut rng);
            let n = match num {
                Some(num) => *num,
                None => 0,
            };
            let _ret = engine.get(&get_test_key(n));
        });
    });
}

fn benchmark_delete(c: &mut Criterion) {
    // 打开存储引擎
    let opts = Options {
        dir_path: PathBuf::from("/tmp/bitcask-rs-bench-delete"),
        ..Default::default()
    };

    let engine = Engine::open(opts).unwrap();

    for i in 0..100000 {
        let ret = engine.put(get_test_key(i), get_test_value(i));
        assert!(ret.is_ok());
    }

    let mut rng = rand::rng();
    let mut nums: Vec<u32> = (0..u32::MAX).collect();

    c.bench_function("bitcask-get-bench", |b| {
        b.iter(|| {
            nums.shuffle(&mut rng);
            let num = nums.choose(&mut rng);
            let n = match num {
                Some(num) => *num,
                None => 0,
            };
            let _ret = engine.delete(&get_test_key(n));
        });
    });
}

criterion_group!(benches, benchmark_put, benchmark_get, benchmark_delete);
criterion_main!(benches);
