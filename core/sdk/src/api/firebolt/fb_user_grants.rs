use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUserGrantsByCapabilityRequest {
    pub capability: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GetUserGrantsByAppRequest {
    pub app_id: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    app: Option<AppInfo>, //None in case of device
    state: String,
    capability: String,
    role: String,
    lifespan: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires: Option<String>, // Option<u64>,
}
