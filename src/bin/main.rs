extern crate shelflife;
extern crate clap;
extern crate dotenv;

use std::env;
use clap::{Arg, App};
use dotenv::dotenv;
use mongodb::ThreadedClient;

use shelflife::{make_api_call,
                query_known_namespace,
                remove_item_from_db_namespace_table,
                view_db_namespace_table,
                Result};

fn main() -> Result<()> {
    dotenv().ok();
    let token = env::var("OKD_TOKEN")?;
    let endpoint = env::var("ENDPOINT")?;
    
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

    let matches = App::new("ShelfLife")
        .version("0.0.5 or something")
        .author("Willard N. <willnilges@mail.rit.edu>")
        .about("Automatic management of spin-down and deletion of OKD projects.")
        .arg(Arg::with_name("delete")
            .short("d")
            .long("delete")
            .value_name("NAMESPACE")
            .help("Deletes a namespace out of MongoDB")
            .takes_value(true))
        .arg(Arg::with_name("known")
            .short("k")
            .long("known")
            .value_name("NAMESPACE")
            .help("Query API and Database for a known namespace")
            .takes_value(true))
        .arg(Arg::with_name("project")
            .short("p")
            .long("project")
            .value_name("NAMESPACE")
            .help("Query API for project info about a namespace")
            .takes_value(true))
        .arg(Arg::with_name("view")
            .short("v")
            .long("view")
            .help("Print namespaces currently tracked in MongoDB"))
        .get_matches();

    if let Some(deleted) = matches.value_of("delete") {
        remove_item_from_db_namespace_table(&mongo_client, deleted)?;
    }
    
    if let Some(known_namespace) = matches.value_of("known") {
        query_known_namespace(&mongo_client, &http_client, &token, &endpoint, known_namespace)?;
    }

    if let Some(project_name) = matches.value_of("project") {
        let call = format!("https://{}/oapi/v1/projects/{}", endpoint, project_name);
        let result = make_api_call(&http_client, &call, &token)?;
        dbg!(result);
    }

    if matches.occurrences_of("view") > 0 {
        view_db_namespace_table(&mongo_client)?;
    }

    Ok(())
}
