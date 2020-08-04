extern crate shelflife;
extern crate clap;
extern crate dotenv;
#[macro_use] extern crate log;

use std::env;
use clap::{Arg, App, AppSettings};
use dotenv::dotenv;
use mongodb::ThreadedClient;

// Logging. Ehh??
// use error_chain::error_chain;
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::config::{Appender, Config, Root};


// error_chain! {
//     foreign_links {
//         Io(std::io::Error);
//         LogConfig(log4rs::config::Errors);
//         SetLogger(log::SetLoggerError);
//     }
// }

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

    //Set up logging.
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {l} - {m}\n")))
        .build(env::var("LOG_PATH").unwrap())?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder()
                   .appender("logfile")
                   .build(LevelFilter::Info))?;

    log4rs::init_config(config)?;

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
    info!("Running ShelfLife...");

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
        .get_matches();

    let mut collection = "graylist";
    if matches.occurrences_of("whitelist") > 0 {
        collection = "whitelist";
        info!("Running in whitelist mode.")
    } else {
        info!("Running in graylist mode.")
    }

    if matches.occurrences_of("all") > 0 {
        info!("Querying OKD API for namespace information...");
        let proj_names = get_namespaces(&http_client);
        for project in proj_names.unwrap() {
            query_known_namespace(&mongo_client, collection, &http_client, &project, true)?;
        }
        info!("OKD Query complete.");
    }
 
    if matches.occurrences_of("cull") > 0 {
        info!("Culling...");
        println!("You might want to run the -a option if you haven't already.");
        let _expiration = check_expiry_dates(&http_client, &mongo_client, collection, false); // 'False' as in DRYRUN IS DISABLED THIS IS ACTUALLY DESTRUCTIVE!
        info!("Cull complete.");
    }

    if matches.occurrences_of("dryrun") > 0 {
        info!("Doing a dryrun cull...");
        let _expiration = check_expiry_dates(&http_client, &mongo_client, collection, true); // This is NOT destructive
        info!("Dryrun cull complete.");
    }

    if let Some(deleted) = matches.value_of("remove") {
        info!("Removing db item: {}", &deleted);
        remove_db_item(&mongo_client, collection, deleted)?;
    }
    
    if let Some(known_namespace) = matches.value_of("known") {
        info!("Querying OKD API for: {}", &known_namespace);
        query_known_namespace(&mongo_client, collection, &http_client, known_namespace, false)?;
    }

    if let Some(project_name) = matches.value_of("project") {
        info!("Querying OKD API for details about: {}", &project_name);
        let call = format!("https://{}/oapi/v1/projects/{}", endpoint, project_name);
        let result = get_call_api(&http_client, &call)?;
        dbg!(result);
    }

    if matches.occurrences_of("list") > 0 {
        view_db(&mongo_client, collection)?;
    }

    Ok(())
}
