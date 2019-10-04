extern crate mongodb;
extern crate reqwest;
#[macro_use]
extern crate prettytable;
pub mod protocol;
extern crate lettre;
extern crate lettre_email;
extern crate dotenv;

use mongodb::db::ThreadedDatabase;
use mongodb::{bson, doc, Bson, ThreadedClient};
use prettytable::Table;
use protocol::*;
use reqwest::StatusCode;
use chrono::{DateTime, Utc};
use lettre::smtp::authentication::{Credentials, Mechanism};
use lettre::{Transport, SmtpClient};
use lettre::smtp::ConnectionReuseParameters;
use lettre_email::Email;
use std::env;
use dotenv::dotenv;
use std::fs;
use std::process::Command;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn query_known_namespace(
    mongo_client: &mongodb::Client,
    collection: &str,
    http_client: &reqwest::Client,
    namespace: &str,
) -> Result<()> {
    println!(
        "{}",
        format!("\nQuerying API for namespace {}...", namespace).to_string()
    );
    let namespace_info = get_shelflife_info(
        http_client,
        namespace,
    )?;
    print!("\n > > > API Response > > > ");
    println!(
        "{} {:?} {} {}",
        namespace_info.name, namespace_info.admins, namespace_info.last_update, namespace_info.cause
    );

    // Query the DB and get back a table of already added namespaces
    let current_table: Vec<DBItem> = get_db(mongo_client, &collection)?;
    
    // Check if the namespace queried for is in the DB, and if not, ask to put it in.
    let queried_namespace = namespace_info.name.to_string();
    if !current_table
        .iter()
        .any(|x| x.name.to_string() == queried_namespace)
    {
        println!(
            "\nThis namespace ({}) is not in the database! Would you like to add it? (y/n): ",
            queried_namespace
        );
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .expect("Could not read response");
        if input.trim() == "y" {
             match collection.as_ref() {
                "namespaces" => {
                    println!("Putting a ShelfLife on {}", queried_namespace);
                }
                "whitelist" => {
                    println!("Whitelisting {}", queried_namespace);
                }
                _ => {
                    println!("Unknown table:");
                }
            }
            let _table_add = add_item_to_db(mongo_client, &collection, namespace_info);
        } else if input.trim() == "n" {
            println!("Ok.");
        } else {
            println!("Invalid response.");
        }
    } else {
        println!("The requested namespace is in the database.")
    }
    Ok(())
}

// Queries the API and returns a Struct with data relevant for shelflife's operation.
fn get_shelflife_info(
    http_client: &reqwest::Client,
    namespace: &str,
) -> Result<DBItem> {
    let endpoint = env::var("ENDPOINT")?; 
    // Query for builds
    // Formulate the call
    let builds_call = format!(
        "https://{}/apis/build.openshift.io/v1/namespaces/{}/builds",
        endpoint, namespace
    );
    let builds_resp = get_call_api(&http_client, &builds_call); // Make the call
    let builds_json: BuildlistResponse = builds_resp?.json()?; // Bind json of reply to struct.
    let mut builds = Vec::new();
    for item in builds_json.items {
        builds.push(DateTime::parse_from_rfc3339(&item.status.completion_timestamp));
    }
    
    // Query deployment configs
    // Formulate the call
    let deploycfgs_call = format!(
        "https://{}/apis/apps.openshift.io/v1/namespaces/{}/deploymentconfigs",
        endpoint, namespace
    );
    let deploycfgs_resp = get_call_api(&http_client, &deploycfgs_call); // Make the call
    let deploycfgs_json: DeploymentResponse = deploycfgs_resp?.json()?; // Bind json of reply to struct.
    // Get the timestamp of the last deployments.
    let mut deploys = Vec::new();
    for config in deploycfgs_json.items {
        for condition in config.status.conditions {
            deploys.push(DateTime::parse_from_rfc3339(&condition.last_update_time));
        }
    }
    
    // Default to using latest deploymentconfig date if there are no builds available,
    // Otherwise compare build dates to see if there's a later one that can be used
    // instead.
    let latest_deploy = deploys.last().unwrap().unwrap();
    let mut latest_update = latest_deploy;
    let mut cause = "Deployment";
    if builds.len() != 0 {
        let latest_build = builds.last().unwrap().unwrap();
        // If the app was deployed after it was built, use the deploy time as the latest
        // update, otherwise, use the build time.
        if latest_deploy.signed_duration_since(latest_build) > chrono::Duration::seconds(0) {
            latest_update = latest_deploy;
            cause = "Deployment";
        } else {
            latest_update = latest_build;
            cause = "Build";
        }
    }

    // Query rolebindings for the admins of the namespace
    let rolebdgs_call = format!(
        "https://{}/apis/authorization.openshift.io/v1/namespaces/{}/rolebindings",
        endpoint, namespace
    );
    let rolebdgs_resp = get_call_api(&http_client, &rolebdgs_call);
    let rolebdgs_json: RolebindingsResponse = rolebdgs_resp?.json()?;
    let rolebdgs: Vec<String> = rolebdgs_json
        .items
        .into_iter()
        .filter(|item| item.metadata.name == "admin")
        .filter_map(|item| item.user_names)
        .flatten()
        .collect();
    // Strip quotation marks off names.
    let mut rolebindings = Vec::new();
    for name in rolebdgs {
        rolebindings.push(name.replace("\"", ""));
    }

    // Build the response struct
    let api_response = DBItem {
        name: namespace.to_string(),
        admins: rolebindings,
        last_update: latest_update.to_rfc2822(),
        cause: cause.to_string(), 
    };
    Ok(api_response)
}

pub fn check_expiry_dates(http_client: &reqwest::Client, mongo_client: &mongodb::Client, collection: &str) -> Result<()>{
    let endpoint = env::var("ENDPOINT")?; 

    let email_srv = env::var("EMAIL_SRV")?;
    let email_uname = env::var("EMAIL_UNAME")?;
    let email_passwd = env::var("EMAIL_PASSWD")?;
    let email_addr = env::var("EMAIL_ADDRESS")?;

    let mut mailer = SmtpClient::new_simple(&email_srv).unwrap()
        .credentials(Credentials::new(email_uname.to_string(), email_passwd.to_string()))
        .smtp_utf8(true)
        .authentication_mechanism(Mechanism::Plain)
        .connection_reuse(ConnectionReuseParameters::ReuseUnlimited).transport();

    let namespaces: Vec<DBItem> = get_db(mongo_client, collection).unwrap();
    for item in namespaces.iter(){
        let last_update = DateTime::parse_from_rfc2822(&item.last_update);
        let _last_update_unwrapped = Some(last_update);

        match last_update {
            Ok(last_update_unwrapped) => {
                let age = Utc::now().signed_duration_since(last_update_unwrapped);
                let addr: &str = &*email_addr;
                if age > chrono::Duration::weeks(24) { // Check longest first, decending.
                    println!("The last update to {} was more than 24 weeks ago. Deleting...", &item.name);
                    println!("But not really because the delete call was commented out!!!");

                    //let delete_call = format!("https://{}/apis/project.openshift.io/v1/projects/{}", endpoint, &item.name);
                    //let _result = delete_call_api(&http_client, &delete_call);

                    //let _db_result = remove_db_item(mongo_client, collection, &item.name);

                    for name in item.admins.iter() {
                        let strpname = name.replace("\"", "");
                        println!("Notifying {}", &strpname);
                        let strpname = name.replace("\"", "");
                        let email = Email::builder()
                            .to((format!("{}@csh.rit.edu", strpname), strpname))
                            .from(addr)
                            .subject("Hi, I nuked your project :)")
                            .text(format!("Hello! You are receiving this message because your OKD project, {}, has now gone more than 24 weeks without an update ({}). It has been deleted from OKD. You can find a backup of the project on your usershare at <link>. Thank you for using ShelfLife, try not to let your pods get too moldy next time.", &item.name, &item.last_update))
                            .build()
                            .unwrap();
                        let _mail_result = mailer.send(email.into());
                    }

                }else if age > chrono::Duration::weeks(16) {
                    println!("The last update to {} was more than 16 weeks ago.", &item.name);
                    println!("Spinning down...");

                    // Query deployment configs that will need to be spun down.
                    let deploycfgs_call = format!(
                        "https://{}/apis/apps.openshift.io/v1/namespaces/{}/deploymentconfigs",
                        endpoint, &item.name
                    );

                    let deploycfgs_resp = get_call_api(&http_client, &deploycfgs_call); // Make the call
                    let deploycfgs_json: DeploymentResponse = deploycfgs_resp?.json()?;
                    let mut deploys = Vec::new();
                    for item in deploycfgs_json.items {
                        if item.status.replicas > 0 {
                            println!("Spinning down {} replicas in {}", &item.status.replicas, &item.metadata.name);
                            deploys.push(item.metadata.name);
                        }
                    }

                    // Tell deploymentconfigs to scale down to 0 pods.
                    for deployment in deploys {
                        let call = format!(
                            "https://{}/oapi/v1/namespaces/{}/deploymentconfigs/{}/scale",
                            endpoint, &item.name, &deployment
                        );
                        let post = format!(
                        "{{\"apiVersion\":\"extensions/v1beta1\",\"kind\":\"Scale\",\"metadata\":{{\"name\":\"{}\",\"namespace\":\"{}\"}},\"spec\":{{\"replicas\":0}}}}",
                        &deployment, &item.name);
                        let _result = put_call_api(&http_client, &call, String::from(&post))?;
                    }

                    println!("Notifying admins...");
                    for name in item.admins.iter() {
                        let strpname = name.replace("\"", "");
                        println!("Notifying {}", &strpname);
                        let email = Email::builder()
                            .to((format!("{}@csh.rit.edu", strpname), strpname))
                            .from(addr)
                            .subject("Your project's resources have been revoked.")
                            .text(format!("Hello! You are receiving this message because your OKD project, {}, has now gone more than 16 weeks without an update ({}). All applications on the project have now been reduced to 0 pods. If you would like to revive it, do so, and its ShelfLife will reset. Otherwise, it will be deleted in another 4 weeks.", &item.name, &item.last_update))
                            .build()
                            .unwrap();
                        let _mail_result = mailer.send(email.into());
                    }
                }else  if age > chrono::Duration::weeks(12) {
                    println!("The last update to {} was more than 12 weeks ago.", &item.name);
                    for name in item.admins.iter() {
                        let strpname = name.replace("\"", "");
                        println!("Notifying {}", &strpname);
                        let email = Email::builder()
                            .to((format!("{}@csh.rit.edu", strpname), strpname))
                            .from(addr)
                            .subject(format!("Old OKD project: {}", &item.name))
                            .text(format!("Hello! You are receiving this message because your OKD project, {}, has gone more than 12 weeks without an update ({}). Please consider updating with a build, deployment, or asking an RTP to put the project on ShelfLife's whitelist. Thanks!.", &item.name, &item.last_update))
                            .build()
                            .unwrap();
                        let _mail_result = mailer.send(email.into());
                    }
                }
            }
            Err(_) => {}
        }
    }
    mailer.close(); 
    Ok(())
}

// Make a call to the Openshift API about some namespace info.
pub fn get_call_api(http_client: &reqwest::Client, call: &str,) -> Result<reqwest::Response> {
    let token = env::var("OKD_TOKEN")?;
    let response = http_client 
        .get(call)
        .header("Authorization", format!("Bearer {}", token))
        .send()?;

    // Ensure the call was successful
    if response.status() == StatusCode::OK {
        Ok(response)
    } else {
        return Err(From::from(format!(
            "Error! Could not run API call. Call: {}, Code: {}", call, response.status()),
        ));
    }
}

pub fn put_call_api(http_client: &reqwest::Client, call: &str, post: String,) -> Result<reqwest::Response> {
    let token = env::var("OKD_TOKEN")?;
    let response = http_client
        .put(call)
        .header("Authorization", format!("Bearer {}", token))
        .body(post)
        .send()?;
     
    // Ensure the call was successful
    if response.status() == StatusCode::OK {
        Ok(response)
    } else {
        return Err(From::from(format!(
            "Error! Could not run API call. Call: {}, Code: {}", call, response.status()),
        ));
    }
}

pub fn delete_call_api(http_client: &reqwest::Client, call: &str,) -> Result<reqwest::Response> {
    let token = env::var("OKD_TOKEN")?;
    let response = http_client
        .delete(call)
        .header("Authorization", format!("Bearer {}", token))
        .send()?;
     
    // Ensure the call was successful
    if response.status() == StatusCode::OK {
        Ok(response)
    } else {
        return Err(From::from(format!(
            "Error! Could not run API call. Call: {}, Code: {}", call, response.status()),
        ));
    }

}

pub fn export_project_sh(project: &str) -> Result<()> {
    let token = env::var("OKD_TOKEN")?;
    let endpoint = env::var("ENDPOINT")?;
    let fail = "failed to execute process";

    Command::new("sh").arg("-c").arg(format!("oc login https://{} --token={}", endpoint, token))
    .current_dir("/tmp/backup_test").status().expect(fail);
    
    Command::new("sh").arg("-c").arg(format!("mkdir /tmp/backup_test/{}", project))
    .current_dir("/tmp/backup_test").output().expect(fail);
    Command::new("sh").arg("-c").arg(format!("oc project {}", project))
    .current_dir("/tmp/backup_test").output().expect(fail);
    Command::new("sh").arg("-c").arg(format!("oc get -o yaml --export all > {}/project.yaml", project))
    .current_dir("/tmp/backup_test").output().expect(fail);
    println!("Done with GET for export all");
    let items = vec!["rolebindings", "serviceaccounts", "secrets", "imagestreamtags", "podpreset", "cms", "egressnetworkpolicies", "rolebindingrestrictions", "limitranges", "resourcequotas", "pvcs", "templates", "cronjobs", "statefulsets", "hpas", "deployments", "replicasets", "poddisruptionbudget", "endpoints"];
    for object in items {
        Command::new("sh").arg("-c").arg(format!("oc get -o yaml --export {} > {}/{}.yaml", object, project, object))
    .current_dir("/tmp/backup_test").output().expect(fail);
        println!("Done with GET for export {}", object);
    }
    Ok(())
}

fn get_db(mongo_client: &mongodb::Client, collection: &str) -> Result<Vec<DBItem>> {
    let coll = mongo_client
        .db("SHELFLIFE_NAMESPACES")
        .collection(&collection);
    let mut namespace_table = Vec::new(); // The vec of namespace information we're gonna send back.

    // Find the document and receive a cursor
    let cursor = coll.find(None, None).unwrap();
    for result in cursor {
        if let Ok(item) = result {
            let mut doc_name = String::new();
            let mut doc_admins: Vec<String> = Vec::new();
            let mut doc_last_deployment = String::new();
            if let Some(&Bson::String(ref name)) = item.get("name") {
                doc_name = name.to_string();
            }
            if let Some(&Bson::Array(ref admins)) = item.get("admins") {
                let doc_admins_bson = admins.to_vec();
                for item in doc_admins_bson {
                    doc_admins.push(item.to_string());
                }
            }
            if let Some(&Bson::String(ref last_deployment)) = item.get("last_update") {
                doc_last_deployment = last_deployment.to_string();
            }
            let namespace_document = DBItem {
                name: doc_name.as_str().to_string(),
                admins: doc_admins,
                last_update: doc_last_deployment,
                cause: "Deployment".to_string(),
            };
            namespace_table.push(namespace_document);
        }
    }
    Ok(namespace_table)
}

pub fn view_db(mongo_client: &mongodb::Client, collection: &str) -> Result<()> {
    // Query the DB and get back a table of already added namespaces
    let current_table: Vec<DBItem> = get_db(mongo_client, collection)?;
    match collection.as_ref() {
        "namespaces" => {
            println!("\nProjects with ShelfLives:");
        }
        "whitelist" => {
            println!("\nWhitelisted projects:");
        }
        _ => {
            println!("\nUnknown table:");
        }
    }
    let mut db_table = Table::new(); // Create the table
    db_table.add_row(row!["Namespace", "Admins", "Latest Update", "Cause"]); // Add a row per time
    for row in &current_table {
        db_table.add_row(row![
            row.name,
            format!("{:?}", row.admins),
            row.last_update,
            row.cause,
        ]);
    }
    db_table.printstd(); // Print the table to stdout
    Ok(())
}


fn add_item_to_db(mongo_client: &mongodb::Client, collection: &str, item: DBItem) -> Result<()> {
    // Direct connection to a server. Will not look for other servers in the topology.
    dbg!(&item.last_update);
    let coll = mongo_client
        .db("SHELFLIFE_NAMESPACES")
        .collection(&collection);
    coll.insert_one(doc!{"name": item.name, "admins": bson::to_bson(&item.admins)?, "last_update": item.last_update, "cause": item.cause}, None).unwrap();
    Ok(())
}


pub fn remove_db_item(mongo_client: &mongodb::Client, collection: &str, namespace: &str) -> Result<()> {
    // Direct connection to a server. Will not look for other servers in the topology.
    let coll = mongo_client
        .db("SHELFLIFE_NAMESPACES")
        .collection(collection);
    coll.find_one_and_delete(doc! {"name": namespace}, None)
        .unwrap();
    println!("{} has been removed.", namespace);
    Ok(())
}
