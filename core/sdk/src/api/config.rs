use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::extn::{
    extn_capability::ExtnCapability,
    extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Config {
    AllDefaultApps,
    DefaultApp,
    DefaultCountryCode,
    DefaultLanguage,
    DefaultLocale,
    DefaultValues,
    SettingsDefaultsPerApp,
    WebSocketEnabled,
    WebSocketGatewayHost,
    InternalWebSocketEnabled,
    InternalWebSocketGatewayHost,
    Platform,
    PlatformParameters,
    Caps,
    DpabPlatform,
    FormFactor,
    Distributor,
    IdSalt,
    AppRetentionPolicy,
    AppLifecyclePolicy,
    ModelFriendlyNames,
    DistributorExperienceId,
    DefaultName,
    DistributorServices,
    CapsRequiringGrant,
    Exclusory,
    DefaultScanTimeout,
}

impl ExtnPayloadProvider for Config {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Config(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Config> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::Config(r) => return Some(r),
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::get_main_target("config".into())
    }
}

#[derive(Debug, Clone)]
pub enum ConfigResponse {
    String(String),
    Boolean(bool),
    Number(u32),
    Value(Value),
    StringMap(HashMap<String, String>),
    List(Vec<String>),
}
