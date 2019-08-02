extern crate shelflife;
extern crate clap;
extern crate dotenv;

use std::env;
use clap::{Arg, App, AppSettings};
use dotenv::dotenv;
use mongodb::ThreadedClient;

use shelflife::{make_api_call,
                query_known_namespace,
                remove_item_from_db_table,
                view_db_table,
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
        .setting(AppSettings::ArgRequiredElseHelp) 
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
        .arg(Arg::with_name("whitelist")
            .short("w")
            .long("whitelist")
            .help("Determines working with the whitelist or the shelflife table."))
        .get_matches();

    let mut collection = "namespaces";
    match matches.occurrences_of("whitelist") {
        0 => {}
        _ => {
            collection = "whitelist";
        }
    }

    match matches.value_of("delete") {
        None => {}
        _ => {
            let _command = remove_item_from_db_table(
                &mongo_client,
                collection.to_string(),
                matches.value_of("delete").unwrap().to_string());
        }
    }
    
    match matches.value_of("known") {
        None => {}
        _ => {
            let namespace = matches.value_of("known").unwrap().to_string();
            let _command = query_known_namespace(
                &mongo_client,
                collection.to_string(),
                &http_client,
                token.to_string(),
                endpoint.to_string(),
                namespace
            );
        }
    }

    match matches.value_of("project") {
        None => {}
        _ => {
            let call = format!("https://{}/oapi/v1/projects/{}", endpoint, &matches.value_of("project").unwrap().to_string());
            let command = make_api_call(
                &http_client,
                call,
                token);
            dbg!(Some(command)); 
        }
    }

    match matches.occurrences_of("view") {
        0 => {}
        _ => {
            let _command = view_db_table(&mongo_client, collection.to_string());       
        }
    }
    Ok(())
}
