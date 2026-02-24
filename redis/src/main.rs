use anyhow::Result;
use bitcask_rs::options::Options;
use redis::types::RedisDataStructure;
use std::{collections::HashMap, sync::Mutex, time::Duration};

const SERVER_ADDR: &str = "127.0.0.1:6380";

type CmdHandler = dyn Fn(&mut redcon::Conn, Vec<Vec<u8>>, &Mutex<RedisDataStructure>);

fn main() -> Result<()> {
    // 打开 Redis 数据服务
    let rds = Mutex::new(RedisDataStructure::new(Options::default())?);

    // 启动 Redis server 服务
    let mut bitcask_server = redcon::listen(SERVER_ADDR, rds)?;
    bitcask_server.command = Some(|conn, rds, args| {
        let name = String::from_utf8_lossy(&args[0]).to_lowercase();
        // 存储支持的命令列表
        let mut supported_commands = HashMap::new();
        supported_commands.insert("set", Box::new(set) as Box<CmdHandler>);
        supported_commands.insert("get", Box::new(get) as Box<CmdHandler>);
        supported_commands.insert("hset", Box::new(hset) as Box<CmdHandler>);
        supported_commands.insert("sadd", Box::new(sadd) as Box<CmdHandler>);
        supported_commands.insert("lpush", Box::new(lpush) as Box<CmdHandler>);
        supported_commands.insert("rpush", Box::new(rpush) as Box<CmdHandler>);
        supported_commands.insert("zadd", Box::new(zadd) as Box<CmdHandler>);

        match supported_commands.get(name.as_str()) {
            Some(handler) => handler(conn, args, rds),
            None => conn.write_error("Err unknown command"),
        }
    });
    println!("Bitacsk serving at {}", bitcask_server.local_addr());
    bitcask_server.serve()?;
    Ok(())
}

fn set(conn: &mut redcon::Conn, args: Vec<Vec<u8>>, rds: &Mutex<RedisDataStructure>) {
    if args.len() != 3 {
        conn.write_error("Err wrong number of arguments");
        return;
    }

    let redis_data_structure = rds.lock().unwrap();
    let ret = redis_data_structure.set(
        &String::from_utf8_lossy(&args[1]),
        &String::from_utf8_lossy(&args[2]),
        Duration::ZERO,
    );
    if ret.is_err() {
        conn.write_error(&ret.err().unwrap().to_string());
    }
    conn.write_string("OK");
}

fn get(conn: &mut redcon::Conn, args: Vec<Vec<u8>>, rds: &Mutex<RedisDataStructure>) {
    if args.len() != 2 {
        conn.write_error("Err wrong number of arguments");
        return;
    }

    let redis_data_structure = rds.lock().unwrap();
    match redis_data_structure.get(&String::from_utf8_lossy(&args[1])) {
        Ok(val) => conn.write_string(val.unwrap().as_str()),
        Err(e) => conn.write_error(&e.to_string()),
    }
}

fn hset(conn: &mut redcon::Conn, args: Vec<Vec<u8>>, rds: &Mutex<RedisDataStructure>) {
    if args.len() != 4 {
        conn.write_error("Err wrong number of arguments");
        return;
    }

    let redis_data_structure = rds.lock().unwrap();
    let key = String::from_utf8_lossy(&args[1]);
    let field = String::from_utf8_lossy(&args[2]);
    let value = String::from_utf8_lossy(&args[3]);

    match redis_data_structure.hset(&key, &field, &value) {
        Ok(val) => conn.write_integer(val as i64),
        Err(e) => conn.write_error(&e.to_string()),
    }
}

fn sadd(conn: &mut redcon::Conn, args: Vec<Vec<u8>>, rds: &Mutex<RedisDataStructure>) {
    if args.len() != 3 {
        conn.write_error("Err wrong number of arguments");
        return;
    }

    let redis_data_structure = rds.lock().unwrap();
    let key = String::from_utf8_lossy(&args[1]);
    let member = String::from_utf8_lossy(&args[2]);

    match redis_data_structure.sadd(&key, &member) {
        Ok(val) => conn.write_integer(val as i64),
        Err(e) => conn.write_error(&e.to_string()),
    }
}

fn lpush(conn: &mut redcon::Conn, args: Vec<Vec<u8>>, rds: &Mutex<RedisDataStructure>) {
    if args.len() != 3 {
        conn.write_error("Err wrong number of arguments");
        return;
    }

    let redis_data_structure = rds.lock().unwrap();
    let key = String::from_utf8_lossy(&args[1]);
    let value = String::from_utf8_lossy(&args[2]);

    match redis_data_structure.lpush(&key, &value) {
        Ok(val) => conn.write_integer(val as i64),
        Err(e) => conn.write_error(&e.to_string()),
    }
}

fn rpush(conn: &mut redcon::Conn, args: Vec<Vec<u8>>, rds: &Mutex<RedisDataStructure>) {
    if args.len() != 3 {
        conn.write_error("Err wrong number of arguments");
        return;
    }

    let redis_data_structure = rds.lock().unwrap();
    let key = String::from_utf8_lossy(&args[1]);
    let value = String::from_utf8_lossy(&args[2]);

    match redis_data_structure.rpush(&key, &value) {
        Ok(val) => conn.write_integer(val as i64),
        Err(e) => conn.write_error(&e.to_string()),
    }
}

fn zadd(conn: &mut redcon::Conn, args: Vec<Vec<u8>>, rds: &Mutex<RedisDataStructure>) {
    if args.len() != 4 {
        conn.write_error("Err wrong number of arguments");
        return;
    }

    let redis_data_structure = rds.lock().unwrap();
    let key = String::from_utf8_lossy(&args[1]);
    let score = String::from_utf8_lossy(&args[2]);
    let member = String::from_utf8_lossy(&args[3]);

    match redis_data_structure.zadd(&key, score.parse().unwrap(), &member) {
        Ok(val) => conn.write_integer(val as i64),
        Err(e) => conn.write_error(&e.to_string()),
    }
}
