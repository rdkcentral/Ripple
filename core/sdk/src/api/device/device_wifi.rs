use crate::extn::{
    extn_capability::{ExtnCapability, ExtnClass},
    extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest}, client::extn_processor::DefaultExtnStreamer,
};

use super::device_request::DeviceRequest;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize)]
#[derive(Clone)]
#[serde(rename_all = "camelCase")]
pub enum WifiSecurityMode {
    None,
    Wep64,
    Wep128,
    WpaPskTkip,
    WpaPskAes,
    Wpa2PskTkip,
    Wpa2PskAes,
    WpaEnterpriseTkip,
    WpaEnterpriseAes,
    Wpa2EnterpriseTkip,
    Wpa2EnterpriseAes,
    Wpa2Psk,
    Wpa2Enterprise,
    Wpa3PskAes,
    Wpa3Sae,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccessPointRequest {
    pub ssid: String,
    pub passphrase: String,
    pub security: WifiSecurityMode,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum WifiRequest {
    Scan,
    Connect(AccessPointRequest),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccessPoint {
    pub ssid: String,
    pub security_mode: WifiSecurityMode,
    pub signal_strength: i32,
    pub frequency: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessPointList {
    pub list: Vec<AccessPoint>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WifiScanRequest {
    pub timeout: Option<u64>,
}

impl Default for WifiScanRequest {
    fn default() -> Self {
        WifiScanRequest { timeout: Some(0) }
    }
}


#[async_trait]
pub trait WifiService {
    async fn scan(self: Box<Self>, ) -> Box<Self>;
}



impl ExtnPayloadProvider for WifiRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Wifi(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::Device(r) => match r {
                    DeviceRequest::Wifi(d) => return Some(d),
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::new_channel(ExtnClass::Device, "wifi".into())
    }
}

