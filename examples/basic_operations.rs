use anyhow::Result;
use bitcask_rs::{db, options::Options};
use bytes::Bytes;

fn main() -> Result<()> {
    let opts = Options::default();
    let engine = db::Engine::open(opts)?;

    let key = Bytes::from("name");
    engine.put(key.clone(), Bytes::from("bitcask-rs"))?;
    let value = engine.get(&key)?;
    println!("value: {:?}", value);

    engine.delete(&key)?;
    if engine.get(&key).is_err() {
        println!("key not found");
    }
    Ok(())
}
