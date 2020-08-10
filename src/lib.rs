extern crate mongodb;
extern crate reqwest;
#[macro_use] extern crate prettytable;
#[macro_use] extern crate log;

pub mod protocol;
extern crate lettre;
extern crate lettre_email;
extern crate dotenv;

use mongodb::db::ThreadedDatabase;
use mongodb::{bson, doc, Bson, ThreadedClient};
use prettytable::Table;
use protocol::*;
use reqwest::StatusCode;
use chrono::{DateTime, Duration, Utc};
use lettre::smtp::authentication::{Credentials, Mechanism};
use lettre::{Transport, SmtpClient};
use lettre::smtp::ConnectionReuseParameters;
use lettre_email::Email;
use std::process::Command;
use std::env;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// Let's make sure those environment variables are set, yea?
pub fn check_env() { // TODO: Actually use results.
    let variables = vec!("OKD_TOKEN", "DB_ADDR", "DB_PORT", "SEND_MAIL", "MAIL_ROOT", "EMAIL_SRV","EMAIL_UNAME", "EMAIL_PASSWD", "EMAIL_ADDRESS", "EMAIL_DOMAIN"); 

    for i in variables {
        match env::var(i) {
            Ok(environment_var) => {
                if environment_var == "" {
                    println!("Can't find {}! This'll produce a TON of errors!", i);
                    error!("Can't find {}! This'll produce a TON of errors!", i);
                    panic!("Refusing to continue without all vars.");
                }
                // TODO: Verbose mode feature.
                //else {
                //    println!("Found {}.", i);
                //}
            },
            Err(e) => {
                eprintln!("Can't find {}! {}", i, e);
                error!("Can't find {}! This'll produce a TON of errors!", i);
                panic!("Refusing to continue without all vars.");
            },
        };
    }
}

/*                                  PROJECT FUNCTIONS  */
/* --------------------------------------------------  */

pub fn get_namespaces(http_client: &reqwest::Client) -> Result<Vec<String>> {
    let endpoint = env::var("ENDPOINT")?; 
    let projects_call = format!("https://{}/apis/project.openshift.io/v1/projects", endpoint); 
    let projects_resp = get_call_api(&http_client, &projects_call);

    match projects_resp {
        Ok(mut call_reply) => {
            let mut projects = Vec::new();
            let projects_json: ProjectResponse = call_reply.json()?;
            
            for item in projects_json.items {
                projects.push(item.metadata.name);
            }
            dbg!(&projects);
            return Ok(projects)
        },
        Err(e) => return Err(e)
    }
}

//Queries API for a project namespace name 
pub fn query_known_namespace(
    mongo_client: &mongodb::Client,
    collection: &str,
    http_client: &reqwest::Client,
    namespace: &str,
    autoadd: bool,
) -> Result<()> {
    // Check the MongoDB to see if we have anything by that name.
    let current_item = match mongo_client
                             .db("SHELFLIFE")
                             .collection(collection)
                             .find_one(Some(doc!{"name": namespace}), None) {
        Ok(Some(db_result)) => {
            println!("{} already discovered.", namespace);
            Some(db_result)
        },
        Ok(None) => {
            println!("{} not yet discovered.", namespace);
            None
        },
        Err(e) => {
            eprintln!("{}", e);
            panic!("Cannot connect to DB!"); // TODO: Consider if we want to panic here.
        }
    };

    // Get all the data we need from the OpenShift API.
    println!("{}",format!("Querying API for namespace {}...", namespace).to_string());
    let mut namespace_info = get_shelflife_info(http_client, namespace,)?;

    // Query the DB and get back a table of already added namespaces
    let current_table: Vec<DBItem> = get_db(mongo_client, &collection)?;
    
    // Check if the namespace queried for is in the DB, and if not, ask to put it in.
    let queried_namespace = namespace_info.name.to_string();
    if !current_table.iter().any(|x| x.name.to_string() == queried_namespace) {
        let mut add = false;
        println!("This namespace ({}) is not in the database. ", queried_namespace);
        info!("Discovered new namespace: {}", &queried_namespace);
        let ignore: Vec<DBItem> = get_db(mongo_client, "ignore")?;
        if collection == "track" {
            if ignore.iter().any(|x| x.name.to_string() == queried_namespace) {
                println!("However, it's ignored. Skipped.");
                warn!("However, it's ignored. Skipped.");
                return Ok(());
            }
            if namespace_info.admins.len() == 0 {
                println!("This namespace has 0 admins. Assuming part of OKD.");
                warn!("This namespace has 0 admins. Assuming part of OKD.");
                return Ok(());
            }
            if namespace_info.name == "management-infra" || namespace_info.name == "default" {
                println!("This looks important. Skipped.");
                warn!("This looks important. Skipped.");
                return Ok(());
            }
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
        } else {
            println!("Adding...")
        }

        if autoadd || add {
            // Get the current time.
            let now = Utc::now();
            let nowrfc = now.to_rfc2822();
            namespace_info.discovery_date = nowrfc;
             match collection.as_ref() {
                "track" => {
                    println!("Tracking {}\n", queried_namespace);
                }
                "ignore" => {
                    println!("Ignoring {}...\n", queried_namespace);
                    print!("Removing theoretical tracking entry... ");
                    let _db_result = remove_db_item(mongo_client, "track", &queried_namespace);
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
        info!("Updated namespace: {}", &queried_namespace);
        // Preserve the discovery date.
        let discovery_date = match current_item {
            Some(item) => item.get_str("discovery_date").unwrap().to_string(),
            None => panic!("How did you get here?"),
        };
        namespace_info.discovery_date = discovery_date; //TODO: I think there's an error to catch here.
        //TODO: Update db entry instead of adding and removing it.
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
    let namespace_call = format!("https://{}/apis/project.openshift.io/v1/projects/{}", endpoint, namespace); // Formulate the call
    let namespace_resp = get_call_api(&http_client, &namespace_call); // Make the call
    let namespace_json: ProjectItem = namespace_resp?.json()?;
    let mut latest_update = DateTime::parse_from_rfc3339(&namespace_json.metadata.creation_timestamp)?;
    let mut cause = "Creation";

    // Query for builds
    let builds_call = format!("https://{}/apis/build.openshift.io/v1/namespaces/{}/builds",endpoint, namespace); // Formulate the call
    let builds_resp = get_call_api(&http_client, &builds_call); // Make the call
    // let builds_json_result = builds_resp?.json(); // Bind json of reply to struct.
    let mut builds = Vec::new();
    // Get the timestamp of the last builds.
    let builds_json: BuildlistResponse = builds_resp?.json()?;
    for item in builds_json.items {
        if let Some(x) = &item.status.completion_timestamp {
            builds.push(DateTime::parse_from_rfc3339(x)?);
        } else {
            println!("Error fetching build timestamp.");
        }
    }
    builds.sort();

    // Query deployment configs
    // Formulate the call
    let deploycfgs_call = format!("https://{}/apis/apps.openshift.io/v1/namespaces/{}/deploymentconfigs", endpoint, namespace);
    let deploycfgs_resp = get_call_api(&http_client, &deploycfgs_call); // Make the call
    let deploycfgs_json: DeploymentResponse = deploycfgs_resp?.json()?; // Bind json of reply to struct.
    // Get the timestamp of the last deployments.
    let mut deploys = Vec::new();
    for config in deploycfgs_json.items {
        for condition in config.status.conditions {
            deploys.push(DateTime::parse_from_rfc3339(&condition.last_update_time)?);
        }
    }
    deploys.sort();

    if deploys.len() > 0 {
        // If it exists, default to using latest deploymentconfig date if there are no
        // builds available.
        latest_update = *deploys.last().unwrap();
        cause = "Deployment";
    }

    if builds.len() > 0 {
        // Compare the latest build date with the current latest update date, which could be
        // either the creation date or the latest deployment date. If the latest build happened
        // later, use that.
        let latest_build = *builds.last().unwrap();
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
        discovery_date: "MISSING!".to_string(), //TODO
        last_update: latest_update.to_rfc2822(),
        cause: cause.to_string(), 
    };
    Ok(api_response)
}

pub fn check_expiry_dates(
    http_client: &reqwest::Client, 
    mongo_client: &mongodb::Client, 
    collection: &str,
    dryrun: bool,
    report: bool,
) -> Result<()>{
    let endpoint = env::var("ENDPOINT")?; 

    let email_srv = env::var("EMAIL_SRV")?;
    let email_uname = env::var("EMAIL_UNAME")?;
    let email_passwd = env::var("EMAIL_PASSWD")?;
    let email_addr = env::var("EMAIL_ADDRESS")?;
    let email_domain = env::var("EMAIL_DOMAIN")?;
    let root_email = env::var("MAIL_ROOT_ADDR")?;

    // See if we should email anyone about what we're doing.
    // This is mostly for development purposes.
    // Please tell your users when you delete their shit.

    let usemail = match env::var("SEND_MAIL")?.as_str() {
        "true" => {
            match dryrun {
                true => false,
                false => {
                    println!("Configured to send mail.");
                    true
                }
            }
        },
        "false" => {
            println!("Configured to NOT send mail. THIS IS REALLY DANGEROUS!");
            false
        },
        _ => {
            println!("Can't find sendmail. Assuming I should NOT send mail!");
            println!("THIS IS REALLY DANGEROUS!");
            false
        },
    };

    // See if we should email root.
    let send_to_root = match env::var("MAIL_ROOT")?.as_str() {
        "true" => {
            match dryrun {
                true => false,
                false => {
                    println!("Configured to send mail to root.");
                    true
                }
            }
        },
        "false" => {
            println!("Configured to NOT send mail to root.");
            false
        },
        _ => {
            println!("Can't find mail_root. Assuming I should NOT send mail to root!");
            false
        },
    };

    println!("Got all env variables.");

    if dryrun {
        println!("We are in DRYRUN MODE! NONE OF SHELFLIFE'S ACTIONS ARE ACTUALLY HAPPENING!");
    }

    let mut report_table = Table::new(); // Create the table for the report

    // Namespace — The namespace
    // Admins — Who owns and operates it
    // Age — How many weeks old it is
    // Action — What ShelfLife is going to do to it
    report_table.add_row(row!["Namespace", "Admins", "Age", "Action"]);

    let addr: &str = &*email_addr;
    let mut mailer = SmtpClient::new_simple(&email_srv).unwrap()
        .credentials(Credentials::new(email_uname.to_string(), email_passwd.to_string()))
        .smtp_utf8(true)
        .authentication_mechanism(Mechanism::Plain)
        .connection_reuse(ConnectionReuseParameters::ReuseUnlimited).transport();

    let namespaces: Vec<DBItem> = get_db(mongo_client, collection).unwrap();
    for item in namespaces.iter(){
        // Compare last update and discovery date and see which one is more recent and go off of that.
        let last_update = DateTime::parse_from_rfc2822(&item.last_update).unwrap();
        let age = match DateTime::parse_from_rfc2822(&item.discovery_date) {
            Ok(date) => {
                let discovery_date = date;
                let since = match last_update.signed_duration_since(discovery_date) {
                    d if d > Duration::nanoseconds(0) => Utc::now().signed_duration_since(last_update),
                    _ => Utc::now().signed_duration_since(discovery_date),
                };
                since
            },
            _ => {
                Utc::now().signed_duration_since(last_update)
            }
        };
        
        print!("Checking status of {}...", &item.name);
        info!("Checking status of {}...", &item.name);

        // TWENTY FOUR WEEKS!
        if age > chrono::Duration::weeks(24) { // Check longest first, decending.
            println!("The last update to {} was more than 24 weeks ago.", &item.name);
            warn!("Age >24 weeks.");
            if report {
                report_table.add_row(row![
                    &item.name,
                    format!("{:?}", item.admins),
                    Duration::num_weeks(&age),
                    "Archive"]);
            }
            if !dryrun {
                println!("Project marked for deletion...");
                println!("Exporting project...");
                let export_result = export_project(&item.name);
                match export_result {
                    Ok(()) => {
                        println!("Export complete.");
                        info!("Exported.")
                    }
                    _ => {
                        println!("Export failed!");
                        error!("Export failed!");
                        dbg!(&export_result);
                    }
                }
                println!("Requesting API to delete...");

                let delete_call = format!("https://{}/apis/project.openshift.io/v1/projects/{}", endpoint, &item.name);
                let _result = delete_call_api(&http_client, &delete_call);
                let _db_result = remove_db_item(mongo_client, collection, &item.name);

                println!("Project has been marked for deletion and removed from ShelfLife DB.");
                info!("Marked for deletion.");

                // Find the names of the admins and send them M A I L!
                if usemail {
                    println!("Notifying admins...");
                    for name in item.admins.iter() {
                        let strpname = name.replace("\"", "");
                        if !send_to_root && &strpname == "root" {
                            println!("I am NOT going to email root.");
                        } else {
                            println!("Notifying {}", &strpname);
                            info!("Notifying {}", &strpname);
                            let strpname = name.replace("\"", "");
                            let email = Email::builder()
                                .to((format!("{}@{}", strpname, email_domain), strpname))
                                .from(addr)
                                .subject("Hi, I nuked your project :)")
                                .text(format!("Hello! You are receiving this message because your OKD project, {}, has now gone more than 24 weeks without an update ({}). It has been deleted from OKD. You can find a backup of the project in your homedir at <link>. Thank you for using ShelfLife, try not to let your pods get too moldy next time.", &item.name, &item.last_update))
                                .build();
                            match email {
                                Err(e) => {
                                    println!("Could not send email. Invalid email address?");
                                    error!("Could not send email.");
                                    eprintln!("{}", e);
                                },
                                _ => {
                                    let _mail_result = mailer.send(email.unwrap().into());
                                }
                            }
                        }
                    }
                }
            }

        }else if age > chrono::Duration::weeks(16) {
            println!("The last update to {} was more than 16 weeks ago.", &item.name);
            warn!("Age >16 weeks.");
            if report {
                report_table.add_row(row![
                    &item.name,
                    format!("{:?}", item.admins),
                    Duration::num_weeks(&age),
                    "Spin-Down"]);
            }
            if !dryrun {
                println!("Spinning down...");
                info!("Spinning down...");

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
                
                if usemail {  
                    // Find the names of the admins and send them M A I L!
                    println!("Notifying admins...");
                    for name in item.admins.iter() {
                        let strpname = name.replace("\"", "");
                        if !send_to_root && &strpname == "root" {
                            println!("I am NOT going to email root.");
                        } else {
                            println!("Notifying {}", &strpname);
                            info!("Notifying {}", &strpname);
                            let email = Email::builder()
                                .to((format!("{}@{}", strpname, email_domain), strpname))
                                .from(addr)
                                .subject("Your project's resources have been revoked.")
                                .text(format!("Hello! You are receiving this message because your OKD project, {}, has now gone more than 16 weeks without an update ({}). All applications on the project have now been reduced to 0 pods. If you would like to revive it, do so, and its ShelfLife will reset. Otherwise, it will be deleted in another 8 weeks.", &item.name, &item.last_update))
                                .build();
                            match email {
                                Err(e) => {
                                    println!("Could not send email. Invalid email address?");
                                    error!("Could not send email.");
                                    eprintln!("{}", e);
                                },
                                _ => {
                                    let _mail_result = mailer.send(email.unwrap().into());
                                }
                            }
                        }
                    }
                }
            }
        }else if age > chrono::Duration::weeks(12) {
            println!("The last update to {} was more than 12 weeks ago.", &item.name);
            warn!("Age >12 weeks.");
            if report {
                report_table.add_row(row![
                    &item.name,
                    format!("{:?}", item.admins),
                    Duration::num_weeks(&age),
                    "Nudge"]);
            }
            if !dryrun && usemail {
                // Find the names of the admins and send them M A I L!
                println!("Notifying admins...");
                for name in item.admins.iter() {
                    let strpname = name.replace("\"", "");
                    if !send_to_root && &strpname == "root" {
                        println!("I am NOT going to email root.");
                    } else {
                        println!("Notifying {}", &strpname);
                        info!("Notifying {}", &strpname);
                        let email = Email::builder()
                            .to((format!("{}@{}", strpname, email_domain), strpname))
                            .from(addr)
                            .subject(format!("Old OKD project: {}", &item.name))
                            .text(format!("Hello! You are receiving this message because your OKD project, {}, has gone more than 12 weeks without an update ({}). Please consider updating with a build, deployment, or asking an RTP to put the project on ShelfLife's ignore. Thanks!.", &item.name, &item.last_update))
                            .build();
                        match email {
                            Err(e) => {
                                println!("Could not send email. Invalid email address?");
                                error!("Could not send email.");
                                eprintln!("{}", e);
                            },
                            _ => {
                                let _mail_result = mailer.send(email.unwrap().into());
                            }
                        }
                    }
                }
            }
        } else {
            println!(" ok.");
        }
    }
    if report {
        let report_message = match dryrun {
            true => "Hello! ShelfLife is going to take the following actions against these projects soon. If this doesn't look right, hop on a console and fix it!",
            false => "Hello! ShelfLife has just taken the following actions against these projects. If something doesn't look right, please direct a complaint to /dev/null on any user machine. Thank you for using ShelfLife! Get a job, or get D E L E T E D.", // TODO: Make these customizable with env variables. (crikey this is really just turning into some kind of config file now, innit?)
        };
        info!("Sending report...");
        println!("Sending report...");
        let email = Email::builder()
            .to((format!("{}@{}", root_email, email_domain), root_email))
            .from(addr)
            .subject(format!("ShelfLife Report"))
            .text(format!("{} \n {}", report_message, report_table.to_string()))
            .build();
        match email {
            Err(e) => {
                println!("Could not send email. Invalid email address?");
                error!("Could not send email.");
                eprintln!("{}", e);
            },
            _ => {
                let _mail_result = mailer.send(email.unwrap().into());
            }
        }
    }

    mailer.close(); 
    println!("Report Sent: ");
    report_table.printstd(); // Print the table to stdout
    Ok(())
}

pub fn export_project(project: &str) -> Result<()> {
    let token = env::var("OKD_TOKEN")?;
    let endpoint = env::var("ENDPOINT")?;
    let fail = "failed to execute process";
    let path = env::var("BACKUP_PATH")?; // One should hope this is somewhere they have write access to.

    // Export project
    Command::new("sh").arg("-c").arg(format!("mkdir {}", &path))
    .current_dir("/").status().expect(fail);
    Command::new("sh").arg("-c").arg(format!("oc login https://{} --token={}", endpoint, token))
    .current_dir(&path).status().expect(fail);
    Command::new("sh").arg("-c").arg(format!("mkdir {}/{}", &path, project))
    .current_dir(&path).output().expect(fail);
    Command::new("sh").arg("-c").arg(format!("oc project {}", project))
    .current_dir(&path).output().expect(fail);
    Command::new("sh").arg("-c").arg(format!("oc get -o yaml --export all > {}/project.yaml", project))
    .current_dir(&path).output().expect(fail);
    println!("Done with GET for export all");
    let items = vec!["rolebindings", "serviceaccounts", "secrets", "imagestreamtags", "podpreset", "cms", "egressnetworkpolicies", "rolebindingrestrictions", "limitranges", "resourcequotas", "pvcs", "templates", "cronjobs", "statefulsets", "hpas", "deployments", "replicasets", "poddisruptionbudget", "endpoints"];
    for object in items {
        Command::new("sh").arg("-c").arg(format!("oc get -o yaml --export {} > {}/{}.yaml", object, project, object))
        .current_dir(&path).output().expect(fail);
        println!("Done with GET for export {}", object);
    }

    //Compress it
    Command::new("sh").arg("-c").arg(format!("zip -r {}.zip {}", project, project))
    .current_dir(&path).output().expect(fail); 
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
        error!("Could not run API call. Call: {}, Code: {}", call, response.status());
        return Err(From::from(format!(
            "Error: Could not run API call. Call: {}, Code: {}", call, response.status()),
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
        error!("Could not run API call. Call: {}, Code: {}", call, response.status());
        return Err(From::from(format!(
            "Error: Could not run API call. Call: {}, Code: {}", call, response.status()),
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
            let mut doc_discovery_date = String::new();
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
            if let Some(&Bson::String(ref discovery_date)) = item.get("discovery_date") {
                doc_discovery_date = discovery_date.to_string();
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
                discovery_date: doc_discovery_date,
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
        "track" => {
            println!("\nTracked projects:");
        }
        "ignore" => {
            println!("\nIgnored projects:");
        }
        _ => {
            println!("\nUnknown table:");
        }
    }
    let mut db_table = Table::new(); // Create the table
    db_table.add_row(row!["Namespace", "Admins", "Discovery Date", "Last Update", "Weeks Spent", "Cause"]); // Add a row per time
    for row in &current_table {
        // This should be safe. Compare discovery date with last update to see
        // which is more recent. Copied and pasted from check_expiry_dates().
        let last_update = DateTime::parse_from_rfc2822(&row.last_update).unwrap();
        let age = match DateTime::parse_from_rfc2822(&row.discovery_date) {
            Ok(date) => {
                let discovery_date = date;
                let since = match last_update.signed_duration_since(discovery_date) {
                    d if d > Duration::nanoseconds(0) => Utc::now().signed_duration_since(last_update),
                    _ => Utc::now().signed_duration_since(discovery_date),
                };
                since
            },
            _ => {
                Utc::now().signed_duration_since(last_update)
            }
        };

        let weeks_since = Duration::num_weeks(&age);

        // Avoid panicking due to string manipulation >_>
        let fmt_disc_date = match row.discovery_date.len() {
            0 => "unknown",
            _ => &row.discovery_date[5..17]
        };

        let fmt_last_update = match row.last_update.len() {
            0 => "unknown",
            _ => &row.last_update[5..17]
        };

        db_table.add_row(row![
            row.name,
            format!("{:?}", row.admins),
            fmt_disc_date,
            fmt_last_update,
            weeks_since,
            row.cause,
        ]);
    }
    db_table.printstd(); // Print the table to stdout
    Ok(())
}

fn add_item_to_db(mongo_client: &mongodb::Client, collection: &str, item: DBItem) -> Result<()> {
    let coll = mongo_client
        .db("SHELFLIFE")
        .collection(&collection);
    coll.insert_one(doc!{"name": item.name,
                         "admins": bson::to_bson(&item.admins)?,
                         "discovery_date": item.discovery_date, 
                         "last_update": item.last_update, 
                         "cause": item.cause}, None)
                         .unwrap();
    Ok(())
}

pub fn remove_db_item(mongo_client: &mongodb::Client, collection: &str, namespace: &str) -> Result<()> {
    let coll = mongo_client
        .db("SHELFLIFE")
        .collection(collection);
    coll.find_one_and_delete(doc!{"name": namespace}, None)
        .unwrap();
    println!("{} has been removed from db.", namespace);
    Ok(())
}
