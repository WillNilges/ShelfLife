use warp::{http, Filter};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

// use mongodb::db::ThreadedDatabase;
use mongodb::{doc, ThreadedClient};
use std::env;

use shelflife::{
    get_db,
};

use shelflife::protocol::DBItem;

type Items = Vec<DBItem>;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Id {
    name: String,
}

// #[derive(Debug, Deserialize, Serialize, Clone)]
// struct Item {
//     namespace: String,
//     admins: Vec<String>,
//     discovery_date: String,
//     last_update: String,
//     cause: String,
// }

#[derive(Clone)]
struct Store {
  graylist: Arc<RwLock<Items>>
}

impl Store {
    fn new() -> Self {
        Store {
            graylist: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

// async fn update_graylist(
//     item: DBItem,
//     store: Store
//     ) -> Result<impl warp::Reply, warp::Rejection> {
//         let name = item.name.clone();
//         store.graylist.write().insert(name, item);
    
//         Ok(warp::reply::with_status(
//             "Added items to the graylist",
//             http::StatusCode::CREATED,
//         ))
// }

// async fn delete_graylist_item(
//     id: Id,
//     store: Store
//     ) -> Result<impl warp::Reply, warp::Rejection> {
//         store.graylist.write().remove(&id.name);
    
//         Ok(warp::reply::with_status(
//             "Removed item from graylist",
//             http::StatusCode::OK,
//         ))
// }

async fn get_graylist(
    store: Store,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        println!("Got a GET request. /v1/graylist");

        let db_addr = &env::var("DB_ADDR").unwrap();
        let db_port = env::var("DB_PORT").unwrap().parse::<u16>().unwrap();
        let mongo_client = mongodb::Client::connect(
            db_addr,
            db_port,
        ).expect("should connect to mongodb");
        let current_table: Vec<DBItem> = get_db(&mongo_client, "graylist").unwrap();
        // let mut result = HashMap::new();
        // for row in current_table {
        // let name = row.name.clone();
        // let item = row.clone();
        // result.insert(name, item);
        // }

        Ok(warp::reply::json(
            &current_table
        ))
}

async fn get_whitelist(
    store: Store,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        println!("Got a GET request. /v1/whitelist");

        let db_addr = &env::var("DB_ADDR").unwrap();
        let db_port = env::var("DB_PORT").unwrap().parse::<u16>().unwrap();
        let mongo_client = mongodb::Client::connect(
            db_addr,
            db_port,
        ).expect("should connect to mongodb");
        let current_table: Vec<DBItem> = get_db(&mongo_client, "whitelist").unwrap();
        // let mut result = HashMap::new();
        // for row in current_table {
        // let name = row.name.clone();
        // let item = row.clone();
        // result.insert(name, item);
        // }

        Ok(warp::reply::json(
            &current_table
        ))
}

// fn delete_json() -> impl Filter<Extract = (Id,), Error = warp::Rejection> + Clone {
//     // When accepting a body, we want a JSON body
//     // (and to reject huge payloads)...
//     warp::body::content_length_limit(1024 * 16).and(warp::body::json())
// }


// fn post_json() -> impl Filter<Extract = (DBItem,), Error = warp::Rejection> + Clone {
//     // When accepting a body, we want a JSON body
//     // (and to reject huge payloads)...
//     warp::body::content_length_limit(1024 * 16).and(warp::body::json())
// }

#[tokio::main]
pub async fn main() {
    let store = Store::new();
    let store_filter = warp::any().map(move || store.clone());

    // let add_items = warp::post()
    //     .and(warp::path("v1"))
    //     .and(warp::path("graylist"))
    //     .and(warp::path::end())
    //     .and(post_json())   
    //     .and(store_filter.clone())
    //     .and_then(update_graylist);

    let get_graylist = warp::get()
        .and(warp::path("v1"))
        .and(warp::path("graylist"))
        .and(warp::path::end())
        .and(store_filter.clone())
        .and_then(get_graylist);

    let get_whitelist = warp::get()
        .and(warp::path("v1"))
        .and(warp::path("whitelist"))
        .and(warp::path::end())
        .and(store_filter.clone())
        .and_then(get_whitelist);

    // let delete_item = warp::delete()
    //     .and(warp::path("v1"))
    //     .and(warp::path("graylist"))
    //     .and(warp::path::end())
    //     .and(delete_json())
    //     .and(store_filter.clone())
    //     .and_then(delete_graylist_item);
    
    // let update_item = warp::put()
    //     .and(warp::path("v1"))
    //     .and(warp::path("graylist"))
    //     .and(warp::path::end())
    //     .and(post_json())
    //     .and(store_filter.clone())
    //     .and_then(update_graylist);

    let cors = warp::cors()
    .allow_any_origin()
    .allow_headers(vec!["User-Agent", "Sec-Fetch-Mode", "Referer", "Origin", "Access-Control-Request-Method", "Access-Control-Request-Headers", "Access-Control-Allow-Origin", "content-type"])
    .allow_methods(vec!["POST", "GET"]);
    
    let routes = get_graylist.or(get_whitelist).with(cors);

    println!("Initialization complete. Serving up the route...");
    warp::serve(routes)
        .run(([0, 0, 0, 0], 3030))
        .await;
}