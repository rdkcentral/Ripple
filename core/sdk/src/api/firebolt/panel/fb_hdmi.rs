use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HdmiSelectOperationResponse {}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HdmiSelectOperationRequest {
    pub port: String,
    pub operation: HdmiOperation,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum HdmiOperation {
    Start,
    Stop,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetAvailableInputsResponse {
    pub devices: Vec<HdmiInput>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HdmiInput {
    pub port: String,
    pub connected: bool,
    pub signal: HDMISignalStatus,
    pub arc_capable: bool,
    pub arc_connected: bool,
    pub edid_version: EDIDVersion,
    pub auto_low_latency_mode_capable: bool,
    pub auto_low_latency_mode_signalled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum HDMISignalStatus {
    None,
    Stable,
    Unstable,
    Unsupported,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum EDIDVersion {
    #[serde(rename = "1.4")]
    Version1_4,
    #[serde(rename = "2.0")]
    Version2_0,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HdmiConnectionChangedInfo {
    pub port: String,
    pub connected: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AutoLowLatencyModeSignalChangedInfo {
    pub port: String,
    pub auto_low_latency_mode_signalled: bool,
}
