extern crate dotenv;
extern crate shelflife;

use dotenv::dotenv;
use mongodb::ThreadedClient;
use std::env;

use shelflife::{query_known_namespace, remove_item_from_db, view_db_namespace_table, Result};

fn main() -> Result<()> {
    dotenv().ok();
    let token = env::var("OKD_TOKEN")?;
    let endpoint = env::var("ENDPOINT")?;
    //let namespace = env::var("TEST_PROJECT")?;

    let http_client = reqwest::Client::new();
    let mongo_client = mongodb::Client::connect(
        &env::var("DB_ADDR")?,
        env::var("DB_PORT")?
            .parse::<u16>()
            .expect("DB_PORT should be an integer"),
    )
    .expect("should connect to mongodb");

    // Friendly and polite greeting...
    println!(
        "{}{}{}",
        "\n      Welcome to ShelfLife     \n",
        "******We nuke old projects******\n",
        " Get a job or get D E L E T E D \n"
    );

    let args: Vec<String> = env::args().collect();
    if args.iter().any(|x| x == "v") {
        let _command = view_db_namespace_table(&mongo_client);
    } else if args.iter().any(|x| x == "k") {
        let namespace = args.last().unwrap().to_string();
        println!("{}", &namespace);
        let _command =
            query_known_namespace(&mongo_client, &http_client, token, endpoint, namespace);
    //dbg!(query.unwrap());
    //else if args.iter().any(|x| x == "s") { sweep_namespaces(token, endpoint); } //WIP
    } else if args.iter().any(|x| x == "d") {
        // If you get a 'd' argument, try to get the next argument after that one and use that to attempt to delete a db item.
        let _command = remove_item_from_db(&mongo_client, args.last().unwrap().to_string());
    } else {
        println!(
            "{}{}",
            "Usage: shelflife [options...] <parameter>\n",
            "    d <namespace>     Delete namespace out of MongoDB\n".to_string()
                + &"    k <namespace>     Query API and Database for a known namespace\n"
                    .to_string()
                + &"    v                 Print namespaces currently tracked in MongoDB"
                    .to_string()
        );
    }
    Ok(())
}
