// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::{MainContract, RippleContract},
};

use super::manifest::{
    app_library::AppLibraryState,
    device_manifest::{LifecyclePolicy, RetentionPolicy},
};

use super::manifest::device_manifest::AppLibraryEntry;

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
    LauncherConfig,
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

    fn contract() -> RippleContract {
        RippleContract::Main(MainContract::Config)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigResponse {
    String(String),
    Boolean(bool),
    Number(u32),
    Value(Value),
    StringMap(HashMap<String, String>),
    List(Vec<String>),
    AllApps(Vec<AppLibraryEntry>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        match payload {
            ExtnPayload::Response(response) => match response {
                ExtnResponse::Value(value) => {
                    if let Ok(v) = serde_json::from_value(value) {
                        return Some(v);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::Main(MainContract::Config)
    }
}
