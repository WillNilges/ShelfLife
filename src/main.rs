extern crate reqwest;
extern crate dotenv;
extern crate mongodb;
pub mod protocol;
use protocol::*;
use dotenv::dotenv;
use std::env;
use chrono::{DateTime};
use mongodb::{bson, doc, Client, ThreadedClient};
use mongodb::db::ThreadedDatabase;

fn main() -> Result<(), Box<std::error::Error>> {
    dotenv().ok();
    let token = env::var("OKD_TOKEN")?;
    let endpoint = env::var("ENDPOINT")?;
    let namespace = env::var("TEST_PROJECT")?;

    // Friendly and polite greeting...
    println!("{}{}{}",
             "     Welcome to Shelf Life!\n",
             "******We nuke old projects******\n",
             " Get a job or get D E L E T E D");

    // Query a project. Use their namespace to get their admin usernames and the last time they were built.
    println!("{}", format!("Querying API for namespace {}...", namespace).to_string());
    let namespace_info = query_api_namespace(token.to_string(), endpoint.to_string(), namespace.to_string())?;
    println!("API Response:");
    println!("{} {:?} {}", namespace_info.name, namespace_info.admins, namespace_info.last_deployment); 

    println!("Current Table of Projects:");
    let current_table: Vec<DBItem> = get_db_namespace_table()?;
    println!("namespace | admins | last_deploy");
   
    for entry in &current_table {
        println!("{} {:?} {}", entry.name, entry.admins, entry.last_deployment);
    }

    println!("Finished :)");
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
    let deploymentconfigs_json: DeploymentResponse = deploymentconfigs_resp.json()?;

    // Query for rolebindings (for the admins of the namespace)
    let rolebindings_call = format!("https://{}/apis/authorization.openshift.io/v1/namespaces/{}/rolebindings", endpoint, namespace);
    let mut rolebindings_resp = client.get(&rolebindings_call)
        .header("Authorization", &token)
        .send()?;
    
    let rolebindings_resp_as_json: RolebindingsResponse = rolebindings_resp.json()?;
    let rolebindings_json_vector: Vec<String> = rolebindings_resp_as_json.items.into_iter()
        .filter(|item| item.metadata.name == "admin")
        .filter_map(|item| item.user_names)
        .flatten()
        .collect();
    let api_response = DBItem{
        name: namespace,
        admins: rolebindings_json_vector,
        last_deployment: deploymentconfigs_json.items[0].metadata.creation_timestamp
    };
    Ok(api_response)
}


fn get_db_namespace_table() -> Result<Vec<DBItem>, Box<std::error::Error>> {
    // Direct connection to a server. Will not look for other servers in the topology.
    let client = Client::connect(&env::var("DB_ADDR")?, env::var("DB_PORT")?.to_string().parse::<u16>().unwrap())
        .expect("Failed to initialize client.");
    let coll = client.db("SHELFLIFE_NAMESPACES").collection("namespaces");

    let mut namespace_table = Vec::new(); // The vec of namespace information we're gonna send back.
    
    // Sample data point used to find real data in the collection.
    let doc = doc! {
        "name" : "test_namespace",
        "admins" : [
            "wilnil",
            "matted"
        ],
        "last_deployment" : "2019-07-25T22:05:53+00:00"
    };

    // Find the document and receive a cursor
    let mut cursor = coll.find(Some(doc.clone()), None)
        .ok().expect("Failed to execute find.");
    while let Some(next) = cursor.next() { // Get next item in the collection
        match next { // Match the response to correct data
            Ok(doc) => {
                // Do some gross bson stuff to get admin Strings out of the query
                let admins_bson = doc.get_array("admins")?.to_vec();
                let mut admins_vec: Vec<String> = Vec::new();
                for item in admins_bson { admins_vec.push(item.to_string()); }

                // Construct a namespace item
                let namespace_item = DBItem {
                    name: doc.get_str("name")?.to_string(),
                    admins: admins_vec, 
                    last_deployment: DateTime::parse_from_rfc3339(doc.get_str("last_deployment")?)?
                };
                namespace_table.push(namespace_item);// Push namespace item to table.
            },
            _ => println!("Error: Could not find data"),
        }    
    }

    Ok(namespace_table)
}
