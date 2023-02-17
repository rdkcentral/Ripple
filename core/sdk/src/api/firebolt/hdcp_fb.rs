use serde::{Deserialize, Serialize};

#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum HdcpProfile {
    #[serde(rename = "hdcp1.4")]
    Hdcp1_4,
    #[serde(rename = "hdcp2.2")]
    Hdcp2_2,
}
