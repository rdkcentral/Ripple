// Copyright 2023 Comcast Cable Communications Management, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

use super::manifest::{
    app_library::AppLibraryState,
    device_manifest::{IdSalt, LifecyclePolicy, RetentionPolicy},
};

use super::manifest::device_manifest::AppLibraryEntry;

pub const FEATURE_CLOUD_PERMISSIONS: &str = "cloud_permissions";

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
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
    LauncherConfig,
    RippleFeatures,
    SavedDir,
    SupportsDistributorSession,
    Firebolt,
    RFC(String),
}

impl ExtnPayloadProvider for Config {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Config(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Config> {
        if let ExtnPayload::Request(ExtnRequest::Config(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Config
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct RfcRequest {
    pub flag: String,
}

impl ExtnPayloadProvider for RfcRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Extn(Value::String(self.flag.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Extn(Value::String(flag))) = payload {
            return Some(RfcRequest { flag });
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::RemoteFeatureControl
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConfigResponse {
    String(String),
    Boolean(bool),
    Number(u32),
    Value(Value),
    StringMap(HashMap<String, String>),
    List(Vec<String>),
    AllApps(Vec<AppLibraryEntry>),
    IdSalt(IdSalt),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct LauncherConfig {
    pub retention_policy: RetentionPolicy,
    pub lifecycle_policy: LifecyclePolicy,
    pub app_library_state: AppLibraryState,
}

impl ExtnPayloadProvider for LauncherConfig {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::Value(value)) = payload {
            if let Ok(v) = serde_json::from_value(value) {
                return Some(v);
            }
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::manifest::device_manifest::{AppManifestLoad, BootState};
    use crate::utils::test_utils::test_extn_payload_provider;
    use std::collections::HashMap;

    #[test]
    fn test_extn_request_config() {
        let contract_type: RippleContract = RippleContract::Config;
        test_extn_payload_provider(Config::DefaultValues, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_rfc_request() {
        let rfc_request = RfcRequest {
            flag: String::from("test_flag"),
        };
        let contract_type: RippleContract = RippleContract::RemoteFeatureControl;
        test_extn_payload_provider(rfc_request, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_launcher_config() {
        let launcher_config = LauncherConfig {
            retention_policy: RetentionPolicy {
                max_retained: 10,
                min_available_mem_kb: 1024,
                always_retained: vec!["app1".to_string(), "app2".to_string()],
            },
            lifecycle_policy: LifecyclePolicy {
                app_ready_timeout_ms: 5000,
                app_finished_timeout_ms: 10000,
            },
            app_library_state: AppLibraryState {
                default_apps: vec![AppLibraryEntry {
                    app_id: "app1".to_string(),
                    manifest: AppManifestLoad::Remote(
                        "https://example.com/app1/manifest".to_string(),
                    ),
                    boot_state: BootState::Inactive,
                }],
                providers: HashMap::new(),
            },
        };
        let contract_type: RippleContract = RippleContract::Config;
        test_extn_payload_provider(launcher_config, contract_type);
    }
}
