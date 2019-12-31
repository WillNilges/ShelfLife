extern crate shelflife;
extern crate clap;
extern crate dotenv;

use std::env;
use clap::{Arg, App, AppSettings};
use dotenv::dotenv;
use mongodb::ThreadedClient;

use shelflife::{
                query_known_namespace,
                check_expiry_dates,
                get_call_api,
                get_project_names,
                remove_db_item,
                view_db,
                Result
            };

fn main() -> Result<()> {
    dotenv().ok();
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
        .author("Willard N. <willnilges@mail.rit.edu>")
        .about("Automatic spin-down and deletion management of OKD projects.")
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(Arg::with_name("all")
            .short("a")
            .long("all")
            .help("Queries all available namespaces and adds/updates any that are missing/outdated to the database."))
        .arg(Arg::with_name("cull")
            .short("c")
            .long("cull")
            .help("Queries all available namespaces and adds/updates any that are missing/outdated to the database and then checks for projects to delete."))
        .arg(Arg::with_name("delete")
            .short("d")
            .long("delete")
            .value_name("NAMESPACE")
            .help("Removes a namespace from the database.")
            .takes_value(true))
        .arg(Arg::with_name("known")
            .short("k")
            .long("known")
            .value_name("NAMESPACE")
            .help("Query API and ShelfLife Database for a known namespace. If it is missing from the database, the user is is asked if they want to add it.")
            .takes_value(true))
        .arg(Arg::with_name("project")
            .short("p")
            .long("project")
            .value_name("NAMESPACE")
            .help("Query API for project info about a namespace.")
            .takes_value(true))
        .arg(Arg::with_name("list")
            .short("l")
            .long("list")
            .help("Print namespaces currently tracked in the database."))
        .arg(Arg::with_name("whitelist")
            .short("w")
            .long("whitelist")
            .help("Enables whitelist mode for that command, performing operations on the whitelist instead of the greylist."))
        .get_matches();

    let mut collection = "graylist";
    if matches.occurrences_of("whitelist") > 0 {
        collection = "whitelist";
    }

    if matches.occurrences_of("all") > 0 {
        let proj_names = get_project_names(&http_client);
        for project in proj_names.unwrap() {
            query_known_namespace(&mongo_client, collection, &http_client, &project, true)?;
        }
    }
 
    if matches.occurrences_of("cull") > 0 {
        let _expiration = check_expiry_dates(&http_client, &mongo_client, collection);
    }

    if let Some(deleted) = matches.value_of("delete") {
        remove_db_item(&mongo_client, collection, deleted)?;
    }
    
    if let Some(known_namespace) = matches.value_of("known") {
        query_known_namespace(&mongo_client, collection, &http_client, known_namespace, false)?;
    }

    if let Some(project_name) = matches.value_of("project") {
        let call = format!("https://{}/oapi/v1/projects/{}", endpoint, project_name);
        let result = get_call_api(&http_client, &call)?;
        dbg!(result);
    }

    if matches.occurrences_of("list") > 0 {
        view_db(&mongo_client, collection)?;
    }

    Ok(())
}
