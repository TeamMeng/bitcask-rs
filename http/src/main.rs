use actix_web::{
    App, HttpResponse, HttpServer, Responder, Scope, delete, get, post,
    web::{self, Bytes},
};
use anyhow::Result;
use bitcask_rs::{db::Engine, errors::AppError, options::Options};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{
    Layer as _, fmt::Layer, layer::SubscriberExt as _, util::SubscriberInitExt as _,
};

const ADDR: &str = "8080";

#[actix_web::main]
async fn main() -> Result<()> {
    let layer = Layer::new().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    let addr = format!("0.0.0.0:{}", ADDR);
    info!("Server listening on: {}", addr);

    // 启动 Engine 实例
    let opts = Options {
        dir_path: PathBuf::from("/tmp/bitcask-rs-http"),
        ..Default::default()
    };
    let engine = Arc::new(Engine::open(opts)?);

    HttpServer::new(move || {
        App::new().app_data(web::Data::new(engine.clone())).service(
            Scope::new("/bitcask")
                .service(put_handler)
                .service(get_handler)
                .service(delete_handler)
                .service(listkeys_handler)
                .service(stat_handler),
        )
    })
    .bind(addr)?
    .run()
    .await?;

    Ok(())
}

#[post("/put")]
async fn put_handler(
    engine: web::Data<Arc<Engine>>,
    data: web::Json<HashMap<String, String>>,
) -> impl Responder {
    for (key, value) in data.iter() {
        if let Err(_) = engine.put(Bytes::from(key.to_string()), Bytes::from(value.to_string())) {
            return HttpResponse::InternalServerError().body("failed to put value in engine");
        }
    }

    HttpResponse::Ok().body("OK")
}

#[get("/get/{key}")]
async fn get_handler(engine: web::Data<Arc<Engine>>, key: web::Path<String>) -> impl Responder {
    let value = match engine.get(&Bytes::from(key.to_string())) {
        Ok(val) => val,
        Err(e) => {
            if e != AppError::KeyNotFound {
                return HttpResponse::InternalServerError().body("failed to get value in engine");
            } else {
                return HttpResponse::Ok().body("key not found");
            }
        }
    };
    HttpResponse::Ok().body(value)
}

#[delete("/delete/{key}")]
async fn delete_handler(engine: web::Data<Arc<Engine>>, key: web::Path<String>) -> impl Responder {
    if let Err(e) = engine.delete(&Bytes::from(key.to_string())) {
        if e != AppError::KeyIsEmpty {
            return HttpResponse::InternalServerError().body("failed to delete value in engine");
        }
    };
    HttpResponse::Ok().body("OK")
}

#[get("/listkeys")]
async fn listkeys_handler(engine: web::Data<Arc<Engine>>) -> impl Responder {
    let keys = engine.list_keys();
    let keys = keys
        .into_iter()
        .map(|key| String::from_utf8(key.to_vec()).unwrap())
        .collect::<Vec<String>>();

    let ret = serde_json::to_string(&keys).unwrap();

    HttpResponse::Ok().body(ret)
}

#[get("/stat")]
async fn stat_handler(engine: web::Data<Arc<Engine>>) -> impl Responder {
    let stat = match engine.stat() {
        Ok(stat) => stat,
        Err(_) => return HttpResponse::InternalServerError().body("failed to get stat in engine"),
    };

    let mut ret = HashMap::new();
    ret.insert("key_num", stat.key_num);
    ret.insert("data_file_num", stat.data_file_num);
    ret.insert("reclaim_size", stat.reclaim_size);
    ret.insert("disk_size", stat.disk_size as _);

    HttpResponse::Ok().body(serde_json::to_string(&ret).unwrap())
}
