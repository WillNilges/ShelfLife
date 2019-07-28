extern crate reqwest;
extern crate dotenv;
extern crate mongodb;
pub mod protocol;
use protocol::*;
use dotenv::dotenv;
use std::{env, io};
use reqwest::StatusCode;
//use chrono::{DateTime, FixedOffset};
use mongodb::{Bson, bson, doc, Client, ThreadedClient};
use mongodb::db::ThreadedDatabase;
#[macro_use] extern crate prettytable;
use prettytable::Table;

fn main() -> Result<(), Box<std::error::Error>> {
    dotenv().ok();
    let token = env::var("OKD_TOKEN")?;
    let endpoint = env::var("ENDPOINT")?;
    let namespace = env::var("TEST_PROJECT")?;

    // Friendly and polite greeting...
    println!("{}{}{}",
           "\n      Welcome to ShelfLife     \n",
             "******We nuke old projects******\n",
             " Get a job or get D E L E T E D \n");

    let args: Vec<String> = env::args().collect();
    if args.iter().any(|x| x == "k"){ let _query = query_known_namespace(token, endpoint, namespace); }
//    else if args.iter().any(|x| x == "s") { sweep_namespaces(token, endpoint); } //WIP
    else {
        println!("{}{}", "Usage: shelflife [options...] <parameter>\n",
                         "    k        Query API and Database for a known namespace");
    }
    println!("Finished :)");
    Ok(())
}


fn query_known_namespace(token: String, endpoint: String, namespace: String) -> Result<(), Box<std::error::Error>>{
    // Query a project. Use their namespace to get their admin usernames and the last time they were built.
    println!("{}", format!("\nQuerying API for namespace {}...", namespace).to_string());
    let namespace_info = query_api_namespace(token.to_string(), endpoint.to_string(), namespace.to_string())?;
    print!("\nAPI Response: ");
    println!("{} {:?} {}", namespace_info.name, namespace_info.admins, namespace_info.last_deployment);
   
    // Query the DB and get back a table of already added namespaces 
    let current_table: Vec<DBItem> = get_db_namespace_table()?;
    println!("\nCurrent Table of Projects:");
    let mut db_table = Table::new(); // Create the table
    db_table.add_row(row!["Namespace", "Admins", "Latest Deployment"]); // Add a row per time
    for row in &current_table {
        db_table.add_row(row![row.name, format!("{:?}", row.admins), row.last_deployment]);
    }
    db_table.printstd(); // Print the table to stdout

    // Check if the namespace queried for is in the DB, and if not, ask to put it in. 
    if !current_table.iter().any(|x| x.name.to_string() == namespace_info.name.to_string()) {
        println!("\nThis namespace is not in the database! Would you like to add it? (y/n): ");
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Could not read response");
        if input.trim() == "y" {
            println!("Putting a ShelfLife on {}", namespace_info.name.to_string());
            let _table_add = add_item_to_db_namespace_table(namespace_info);
        }else if input.trim() == "n" { println!("Ok."); } else { println!("Invalid response."); }
    } else { println!("The requested namespace is in the database.") }
    Ok(())
}


fn query_api_namespace(token: String, endpoint: String, namespace: String) -> Result<DBItem, Box<std::error::Error>> {
    let client = reqwest::Client::new();
    // Query for deployment configs (for their build dates)
    let deploymentconfigs_call = format!("https://{}/oapi/v1/namespaces/{}/deploymentconfigs", endpoint, namespace);
    let token = format!("Bearer {}", token);
    let mut deploymentconfigs_resp = client.get(&deploymentconfigs_call)
        .header("Authorization", &token)
        .send()?;
    match deploymentconfigs_resp.status() {
        StatusCode::OK => {},
        _ => return Err(From::from("Error! Could not fetch deployment configs. Is the namespace wrong?")),
    }
    let deploymentconfigs_json: DeploymentResponse = deploymentconfigs_resp.json()?;
    // Get all of the deployment dates of all of the deployments.
    let mut last_deployments = Vec::new();
    for config in deploymentconfigs_json.items {
        last_deployments.push(config.metadata.creation_timestamp);
    }
 
    // Query for rolebindings (for the admins of the namespace)
    let rolebindings_call = format!("https://{}/apis/authorization.openshift.io/v1/namespaces/{}/rolebindings", endpoint, namespace);
    let mut rolebindings_resp = client.get(&rolebindings_call)
        .header("Authorization", &token)
        .send()?;
    
    match rolebindings_resp.status() {
        StatusCode::OK => {},
        _ => return Err(From::from("Error! Could not fetch rolebindings for deployment. Is the namespace wrong?")),
    } 
    let rolebindings_resp_as_json: RolebindingsResponse = rolebindings_resp.json()?;
    let rolebindings_json_vector: Vec<String> = rolebindings_resp_as_json.items.into_iter()
        .filter(|item| item.metadata.name == "admin")
        .filter_map(|item| item.user_names)
        .flatten()
        .collect();

    // Build the API response 
    let api_response = DBItem{
        name: namespace,
        admins: rolebindings_json_vector,
        last_deployment: last_deployments.first().unwrap().to_string()
    };
    Ok(api_response)
}


fn get_db_namespace_table() -> Result<Vec<DBItem>, Box<std::error::Error>> {
    // Direct connection to a server. Will not look for other servers in the topology.
    let client = Client::connect(&env::var("DB_ADDR")?, env::var("DB_PORT")?.to_string().parse::<u16>().unwrap())
        .expect("Failed to initialize client.");
    let coll = client.db("SHELFLIFE_NAMESPACES").collection("namespaces");

    let mut namespace_table = Vec::new(); // The vec of namespace information we're gonna send back.
    
    // Find the document and receive a cursor
    let cursor = coll.find(None, None).unwrap();
    for result in cursor {
        if let Ok(item) = result {
            let mut doc_name = String::new();
            let mut doc_admins: Vec<String> = Vec::new();
            let mut doc_last_deployment = String::new();
            if let Some(&Bson::String(ref name)) = item.get("name"){
                  doc_name = name.to_string();
             }
            if let Some(&Bson::Array(ref admins)) = item.get("admins") {
                let doc_admins_bson = admins.to_vec();
                for item in doc_admins_bson { doc_admins.push(item.to_string()); }
            }
            if let Some(&Bson::String(ref last_deployment)) = item.get("last_deployment") {
                doc_last_deployment = last_deployment.to_string();
            }
            let namespace_document =  DBItem {
                name: doc_name.as_str().to_string(),
                admins: doc_admins,
                last_deployment: doc_last_deployment
            };
            namespace_table.push(namespace_document);
        }
    }
    Ok(namespace_table)
}


fn add_item_to_db_namespace_table(item: DBItem) -> Result<(), Box<std::error::Error>> {
    // Direct connection to a server. Will not look for other servers in the topology.
    let client = Client::connect(&env::var("DB_ADDR")?, env::var("DB_PORT")?.to_string().parse::<u16>().unwrap())
        .expect("Failed to initialize client.");
    let coll = client.db("SHELFLIFE_NAMESPACES").collection("namespaces");
    coll.insert_one(doc!{"name": item.name, "admins": bson::to_bson(&item.admins)?, "last_deployment": item.last_deployment}, None).unwrap();
    Ok(())
}
