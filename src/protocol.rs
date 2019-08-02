use serde::Deserialize;
//use chrono::{DateTime, FixedOffset};

// Structs for Build
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildStatus {
    pub completion_timestamp: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildItem {
    pub status: BuildStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildlistResponse {
    pub items: Vec<BuildItem>,
}

// Structs for Deployment
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentCondition {
    pub last_update_time: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentStatus {
    pub conditions: Vec<DeploymentCondition>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentItem {
    pub status: DeploymentStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentResponse {
    pub items: Vec<DeploymentItem>,
}

// Structs for Rolebindings
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolebindingsMetadata {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolebindingsItem {
    pub metadata: RolebindingsMetadata,
    pub user_names: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolebindingsResponse {
    pub items: Vec<RolebindingsItem>,
}

// Struct to represent a DB Object
pub struct DBItem {
    pub name: String,
    pub admins: Vec<String>,
    pub last_deployment: String,
}
