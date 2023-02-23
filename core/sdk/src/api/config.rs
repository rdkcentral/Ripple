use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Clone)]
pub enum ConfigResponse {
    String(String),
    Boolean(bool),
    Number(u32),
    Value(Value),
    StringMap(HashMap<String, String>),
    List(Vec<String>),
}
