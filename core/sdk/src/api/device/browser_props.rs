use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub struct BrowserProps {
    pub user_agent: Option<String>,
    pub http_cookie_accept_policy: Option<String>,
    pub local_storage_enabled: Option<bool>,
    pub languages: Option<String>,
    pub headers: Option<String>,
}
