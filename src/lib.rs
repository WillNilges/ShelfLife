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
use std::process::Command;
use std::env;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/*                                  PROJECT FUNCTIONS  */
/* --------------------------------------------------  */

pub fn get_project_names(http_client: &reqwest::Client) -> Result<Vec<String>> {
    let endpoint = env::var("ENDPOINT")?; 
    let projects_call = format!("https://{}/apis/project.openshift.io/v1/projects", endpoint); 
    let projects_resp = get_call_api(&http_client, &projects_call);
    let _projects_resp_unwrapped = Some(&projects_resp);
    let mut projects = Vec::new();
    match projects_resp {
        Ok(mut projects_resp_unwrapped) => {
            let projects_json: ProjectResponse = projects_resp_unwrapped.json()?;
            
            for item in projects_json.items {
                projects.push(item.metadata.name);
            }
            dbg!(&projects);
        },
        Err(_) => {
            dbg!(&projects_resp);
        },
    }
    Ok(projects)
}

//Queries API for a project namespace name 
pub fn query_known_namespace(
    mongo_client: &mongodb::Client,
    collection: &str,
    http_client: &reqwest::Client,
    namespace: &str,
    autoadd: bool,
) -> Result<()> {
    print!("{}",format!("\nQuerying API for namespace {}...", namespace).to_string());
    let namespace_info = get_shelflife_info(http_client, namespace,)?;
    print!(" API Response: ");
    println!("{} {:?} {} {}", namespace_info.name, namespace_info.admins, namespace_info.last_update, namespace_info.cause);

    // Query the DB and get back a table of already added namespaces
    let current_table: Vec<DBItem> = get_db(mongo_client, &collection)?;
    
    // Check if the namespace queried for is in the DB, and if not, ask to put it in.
    let queried_namespace = namespace_info.name.to_string();
    if !current_table.iter().any(|x| x.name.to_string() == queried_namespace) {
        let mut add = false;
        println!("This namespace ({}) is not in the database. ", queried_namespace);
        let whitelist: Vec<DBItem> = get_db(mongo_client, "whitelist")?;
        if whitelist.iter().any(|x| x.name.to_string() == queried_namespace) && collection == "graylist".to_string() {
            println!("However, it's whitelisted. Skipping...");
            return Ok(());
        }
        if !autoadd {
            println!("Would you like to add it? (y/n): ");
            let mut input = String::new();
            while !add {
                std::io::stdin().read_line(&mut input).expect("Could not read response");
                if input.trim() == "y".to_string() {
                    add = true;
                } else if input.trim() == "n".to_string() {
                    println!("Ok. Not adding.");
                    return Ok(());
                } else {
                    println!("Invalid response.");
                    dbg!(&input.trim());
                }
            }
        } else {println!("Adding...")}
        
        if autoadd || add {
             match collection.as_ref() {
                "graylist" => {
                    println!("Graylisting {}\n", queried_namespace);
                }
                "whitelist" => {
                    println!("Whitelisting {}...\n", queried_namespace);
                    print!("Removing theoretical greylist entry... ");
                    let _db_result = remove_db_item(mongo_client, "graylist", &queried_namespace);
                }
                _ => {
                    println!("Unknown table:\n");
                }
            }
            let _table_add = add_item_to_db(mongo_client, &collection, namespace_info);
        } else {
            println!("Invalid response.");
        }
    } else {
        println!("The requested namespace is in the database. Updating entry...");
        let _db_result = remove_db_item(mongo_client, collection, &queried_namespace);
        let _table_add = add_item_to_db(mongo_client, &collection, namespace_info);
        println!("Entry updated.");
    }
    Ok(())
}

// Queries the API and returns a Struct with data relevant for shelflife's operation.
fn get_shelflife_info(
    http_client: &reqwest::Client,
    namespace: &str,
) -> Result<DBItem> {
    let endpoint = env::var("ENDPOINT")?; 

    // Query for creation date. This is guaranteed to exist.
    let project_call = format!("https://{}/apis/project.openshift.io/v1/projects/{}", endpoint, namespace); // Formulate the call
    let project_resp = get_call_api(&http_client, &project_call); // Make the call
    //dbg!(&project_resp);
    let project_json: ProjectItem = project_resp?.json()?;
    let mut latest_update = DateTime::parse_from_rfc3339(&project_json.metadata.creation_timestamp)?;
    let mut cause = "Creation";

    // Query for builds
    let builds_call = format!("https://{}/apis/build.openshift.io/v1/namespaces/{}/builds",endpoint, namespace); // Formulate the call
    let builds_resp = get_call_api(&http_client, &builds_call); // Make the call
    let builds_json: BuildlistResponse = builds_resp?.json()?; // Bind json of reply to struct.
    let mut builds = Vec::new();
    for item in builds_json.items {
        builds.push(DateTime::parse_from_rfc3339(&item.status.completion_timestamp));
    }
    
    // Query deployment configs
    // Formulate the call
    let deploycfgs_call = format!("https://{}/apis/apps.openshift.io/v1/namespaces/{}/deploymentconfigs", endpoint, namespace);
    let deploycfgs_resp = get_call_api(&http_client, &deploycfgs_call); // Make the call
    let deploycfgs_json: DeploymentResponse = deploycfgs_resp?.json()?; // Bind json of reply to struct.
    // Get the timestamp of the last deployments.
    let mut deploys = Vec::new();
    for config in deploycfgs_json.items {
        for condition in config.status.conditions {
            deploys.push(DateTime::parse_from_rfc3339(&condition.last_update_time));
        }
    }

    if deploys.len() > 0 {
        // If it exists, default to using latest deploymentconfig date if there are no
        // builds available.
        let latest_deploy = deploys.last().unwrap().unwrap();
        latest_update = latest_deploy;
        cause = "Deployment";
    }

    if builds.len() != 0 {
        // Compare the latest build date with the current latest update date, which could be
        // either the creation date or the latest deployment date. If the latest build happened
        // later, use that.
        let latest_build = builds.last().unwrap().unwrap();
        if latest_update.signed_duration_since(latest_build) < chrono::Duration::seconds(0) {
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

pub fn check_expiry_dates(
    http_client: &reqwest::Client, 
    mongo_client: &mongodb::Client, 
    collection: &str
) -> Result<()>{
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

        print!("Checking status of {}...", &item.name);

        match last_update {
            Ok(last_update_unwrapped) => {
                let age = Utc::now().signed_duration_since(last_update_unwrapped);
                let addr: &str = &*email_addr;
                if age > chrono::Duration::weeks(24) { // Check longest first, decending.
                    println!("The last update to {} was more than 24 weeks ago. Project marked for deletion...", &item.name);
                    println!("Exporting project...");
                    let export_result = export_project(&item.name);
                    match export_result {
                        Ok(()) => {
                            println!("Export complete.");
                        }
                        _ => {
                            println!("Export failed!");
                            dbg!(&export_result);
                        }
                    }
                    println!("Requesting API to delete...");

                    let delete_call = format!("https://{}/apis/project.openshift.io/v1/projects/{}", endpoint, &item.name);
                    let _result = delete_call_api(&http_client, &delete_call);
                    let _db_result = remove_db_item(mongo_client, collection, &item.name);

                    println!("Project has been marked for deletion and removed from ShelfLife DB.");

                    for name in item.admins.iter() {
                        let strpname = name.replace("\"", "");
                        println!("Notifying {}", &strpname);
                        let strpname = name.replace("\"", "");
                        let email = Email::builder()
                            .to((format!("{}@csh.rit.edu", strpname), strpname))
                            .from(addr)
                            .subject("Hi, I nuked your project :)")
                            .text(format!("Hello! You are receiving this message because your OKD project, {}, has now gone more than 24 weeks without an update ({}). It has been deleted from OKD. You can find a backup of the project in your homedir at <link>. Thank you for using ShelfLife, try not to let your pods get too moldy next time.", &item.name, &item.last_update))
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
                            .text(format!("Hello! You are receiving this message because your OKD project, {}, has now gone more than 16 weeks without an update ({}). All applications on the project have now been reduced to 0 pods. If you would like to revive it, do so, and its ShelfLife will reset. Otherwise, it will be deleted in another 8 weeks.", &item.name, &item.last_update))
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
                } else {
                    println!(" ok.");
                }
            }
            Err(_) => {}
        }
    }
    mailer.close(); 
    Ok(())
}

pub fn export_project(project: &str) -> Result<()> {
    let token = env::var("OKD_TOKEN")?;
    let endpoint = env::var("ENDPOINT")?;
    let fail = "failed to execute process";
    let path = "~/shelflife_backup";

    // Export project
    Command::new("sh").arg("-c").arg("mkdir ~/shelflife_backup")
    .current_dir("~/").status().expect(fail);
    Command::new("sh").arg("-c").arg(format!("oc login https://{} --token={}", endpoint, token))
    .current_dir(path).status().expect(fail);
    Command::new("sh").arg("-c").arg(format!("mkdir {}/{}", path, project))
    .current_dir(path).output().expect(fail);
    Command::new("sh").arg("-c").arg(format!("oc project {}", project))
    .current_dir(path).output().expect(fail);
    Command::new("sh").arg("-c").arg(format!("oc get -o yaml --export all > {}/project.yaml", project))
    .current_dir(path).output().expect(fail);
    println!("Done with GET for export all");
    let items = vec!["rolebindings", "serviceaccounts", "secrets", "imagestreamtags", "podpreset", "cms", "egressnetworkpolicies", "rolebindingrestrictions", "limitranges", "resourcequotas", "pvcs", "templates", "cronjobs", "statefulsets", "hpas", "deployments", "replicasets", "poddisruptionbudget", "endpoints"];
    for object in items {
        Command::new("sh").arg("-c").arg(format!("oc get -o yaml --export {} > {}/{}.yaml", object, project, object))
    .current_dir("/tmp/backup_test").output().expect(fail);
        println!("Done with GET for export {}", object);
    }

    //Compress it
    Command::new("sh").arg("-c").arg(format!("zip -r {}.zip {}", project, project)).current_dir("/tmp/backup_test").output().expect(fail); 
    Ok(())
}

/*                                       API FUNCTIONS  */
/*  --------------------------------------------------  */

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

/*                                  DATABASE FUNCTIONS  */
/*  --------------------------------------------------  */

fn get_db(mongo_client: &mongodb::Client, collection: &str) -> Result<Vec<DBItem>> {
    let coll = mongo_client
        .db("SHELFLIFE")
        .collection(&collection);
    let mut namespace_table = Vec::new(); // The vec of namespace information we're gonna send back.

    // Find the document and receive a cursor
    let cursor = coll.find(None, None).unwrap();
    for result in cursor {
        if let Ok(item) = result {
            let mut doc_name = String::new();
            let mut doc_admins: Vec<String> = Vec::new();
            let mut doc_last_deployment = String::new();
            let mut doc_cause = String::new();
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
            if let Some(&Bson::String(ref cause)) = item.get("cause") {
                doc_cause = cause.to_string();
            }
            let namespace_document = DBItem {
                name: doc_name.as_str().to_string(),
                admins: doc_admins,
                last_update: doc_last_deployment,
                cause: doc_cause.to_string(),
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
        "graylist" => {
            println!("\nGraylisted projects:");
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
        .db("SHELFLIFE")
        .collection(&collection);
    coll.insert_one(doc!{"name": item.name, "admins": bson::to_bson(&item.admins)?, "last_update": item.last_update, "cause": item.cause}, None).unwrap();
    Ok(())
}

pub fn remove_db_item(mongo_client: &mongodb::Client, collection: &str, namespace: &str) -> Result<()> {
    // Direct connection to a server. Will not look for other servers in the topology.
    let coll = mongo_client
        .db("SHELFLIFE")
        .collection(collection);
    coll.find_one_and_delete(doc! {"name": namespace}, None)
        .unwrap();
    println!("{} has been removed.", namespace);
    Ok(())
}
