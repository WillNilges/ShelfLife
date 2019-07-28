use serde::Deserialize;
use chrono::{DateTime, FixedOffset};

// Structs for Deployment
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentMetadata { pub creation_timestamp: DateTime<FixedOffset>}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentItem { pub metadata: DeploymentMetadata }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentResponse { pub items: Vec<DeploymentItem> }

// Structs for Rolebindings
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolebindingsMetadata { pub name: String }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolebindingsItem {
    pub metadata: RolebindingsMetadata,
    pub user_names: Option<Vec<String>>
} 

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolebindingsResponse { pub items: Vec<RolebindingsItem> }

// Struct to represent a DB Object
pub struct DBItem {
    pub name: String,
    pub admins: Vec<String>,
    pub last_deployment: DateTime<FixedOffset>
}
