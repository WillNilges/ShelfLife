extern crate shelflife;
extern crate clap;
extern crate dotenv;
#[macro_use]
extern crate lazy_static;
use regex::Regex;

use std::{env, io};
use std::io::Write;
use clap::{SubCommand, Arg, App, AppSettings};
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

// Thanks Nick!
lazy_static! {
    static ref WORD: Regex = Regex::new(r#"(?m)(?:"([^"]*)")|([^\s]+)"#).unwrap();
}

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
    loop {
        // Print command prompt and get command
        print!("> ");
        io::stdout().flush().expect("Couldn't flush stdout");
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Error reading input.");
        let args = WORD.captures_iter(&input)
                   .map(|cap| cap.get(1).or(cap.get(2)).unwrap().as_str())
                   .collect::<Vec<&str>>();
        //dbg!(&args);
        //println!("{}", args[0]);

        let matches = App::new("shelflife")
            .author("Willard N. <willnilges@mail.rit.edu>")
            .about("Automatic spin-down and deletion management of OKD projects.")
            .setting(AppSettings::NoBinaryName)
            .setting(AppSettings::ArgRequiredElseHelp)
            .subcommand(SubCommand::with_name("all")
                .help("Queries all available namespaces and adds/updates any that are missing/outdated to the database."))
            .subcommand(SubCommand::with_name("cull")
                .help("Checks greylist for projects that need attention. Takes appropriate course of action."))
            .subcommand(SubCommand::with_name("list")
                .help("Print namespaces currently tracked in the database."))
            .subcommand(SubCommand::with_name("exit")
                .help("Leave the CLI"))
            .subcommand(SubCommand::with_name("delete") 
                .help("Removes a namespace from the database.")
                .arg(Arg::with_name("namespace")
                    .short("n")             
                    .long("namespace")      
                    .takes_value(true)
                )
            ) 
            .subcommand(SubCommand::with_name("known")
                .help("Query API and ShelfLife Database for a known namespace. If it is missing from the database, the user is is asked if they want to add it.")
                .arg(Arg::with_name("namespace")
                    .short("n")
                    .long("namespace")
                    .takes_value(true)
                )
            )
            .subcommand(SubCommand::with_name("project")
                .help("Query API for project info about a namespace.")
                .arg(Arg::with_name("namespace")
                    .short("n")
                    .long("namespace")
                    .takes_value(true)
                )
            )
           .arg(Arg::with_name("whitelist")
               .short("w")
               .long("whitelist")
               .help("Enables whitelist mode for that command, performing operations on the whitelist instead of the greylist."))
           .get_matches_from(args);

        //dbg!(&matches);

        let mut collection = "graylist";
        if matches.occurrences_of("whitelist") > 0 {
            collection = "whitelist";
        }

        if let Some(_subcommand) = matches.subcommand_matches("all") {
            let proj_names = get_project_names(&http_client);
            for project in proj_names.unwrap() {
                query_known_namespace(&mongo_client, collection, &http_client, &project, true)?;
            }
        }
     
        if let Some(_subcommand) = matches.subcommand_matches("cull") {
            let _expiration = check_expiry_dates(&http_client, &mongo_client, collection);
        }
        
        if let Some(_subcommand) = matches.subcommand_matches("list") {
            view_db(&mongo_client, collection)?;
        }

        if let Some(_subcommand) = matches.subcommand_matches("exit") {
            println!("Bye!");
            break;
        }
        
        if let Some(subcommand) = matches.subcommand_matches("delete") {
           dbg!(subcommand.is_present("namespace"));
           dbg!(subcommand.value_of("namespace")); 
            if let Some(deleted) = subcommand.value_of("namespace") {
                remove_db_item(&mongo_client, collection, deleted)?;
            }
        }
        
        if let Some(known_namespace) = matches.value_of("known") {
            query_known_namespace(&mongo_client, collection, &http_client, known_namespace, false)?;
        }

        if let Some(project_name) = matches.value_of("project") {
            let call = format!("https://{}/oapi/v1/projects/{}", endpoint, project_name);
            let result = get_call_api(&http_client, &call)?;
            dbg!(result);
        }
    }
Ok(())
}
