use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum AVInputAdjective {
    Hdmi,
    Ota,
    Composite,
}
