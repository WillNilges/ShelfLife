extern crate shelflife;
extern crate clap;
extern crate dotenv;

use std::env;
use clap::{Arg, App, AppSettings};
use dotenv::dotenv;
use mongodb::ThreadedClient;

mod api;

use shelflife::{
    check_env,
    query_known_namespace,
    check_expiry_dates,
    get_call_api,
    get_namespaces,
    remove_db_item,
    view_db,
    Result
};

fn main() -> Result<()> {
    //TODO: Investigate if this is the best way to go about
    // figuring out if env variables exist.
    dotenv().ok();
    check_env();
    let endpoint = env::var("ENDPOINT")?;
    
    let http_client = match env::var("DANGER_ACCEPT_INVALID_CERTS")?.as_str() {
        "true" => {
            reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?
        },
        _ => { // This code sucks.
            reqwest::Client::new()
        },
    };

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
        .author("Will N. <willnilges@mail.rit.edu>")
        .about("Automatic spin-down and deletion management of OKD projects.")
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(Arg::with_name("all")
            .short("a")
            .long("all")
            .help("Queries all available namespaces and adds/updates any that are missing/outdated to the database."))
        .arg(Arg::with_name("cull")
            .short("c")
            .long("cull")
            .help("Checks graylist for projects that need attention. Takes appropriate course of action."))
        .arg(Arg::with_name("dryrun")
            .short("d")
            .long("dryrun")
            .help("Checks graylist for projects that need attention. Takes no action."))
        .arg(Arg::with_name("remove")
            .short("r")
            .long("remove")
            .value_name("NAMESPACE")
            .help("Removes a namespace from the database.")
            .takes_value(true))
        .arg(Arg::with_name("known")
            .short("k")
            .long("known")
            .value_name("NAMESPACE")
            .help("Query API and ShelfLife Database for a known namespace. If it is missing from the database, the user is asked if they want to add it.")
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
        .arg(Arg::with_name("service")
            .short("s")
            .long("service")
            .help("Starts the API service."))
        .get_matches();

    let mut collection = "graylist";
    if matches.occurrences_of("whitelist") > 0 {
        collection = "whitelist";
    }

    if matches.occurrences_of("all") > 0 {
        let proj_names = get_namespaces(&http_client);
        for project in proj_names.unwrap() {
            query_known_namespace(&mongo_client, collection, &http_client, &project, true)?;
        }
    }
 
    if matches.occurrences_of("cull") > 0 {
        println!("You might want to run the -a option if you haven't already.");
        let _expiration = check_expiry_dates(&http_client, &mongo_client, collection, false); // 'False' as in DRYRUN IS DISABLED THIS IS ACTUALLY DESTRUCTIVE!
    }

    if matches.occurrences_of("dryrun") > 0 {
        let _expiration = check_expiry_dates(&http_client, &mongo_client, collection, true); // This is NOT destructive
    }

    if let Some(deleted) = matches.value_of("remove") {
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

    if matches.occurrences_of("service") > 0 {
        api::main();
    }

    Ok(())
}
