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
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
    utils::error::RippleError,
};

use super::gateway::rpc_gateway_api::CallContext;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum SettingKey {
    VoiceGuidanceEnabled,
    ClosedCaptions,
    AllowPersonalization,
    AllowWatchHistory,
    ShareWatchHistory,
    DeviceName,
    PowerSaving,
    LegacyMiniGuide,
}

impl ToString for SettingKey {
    fn to_string(&self) -> String {
        let s = serde_json::to_string(self).unwrap();
        s[1..s.len() - 1].into()
    }
}

impl FromStr for SettingKey {
    type Err = RippleError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let result = serde_json::from_str(s);

        if let Ok(v) = result {
            Ok(v)
        } else {
            Err(RippleError::ParseError)
        }
    }
}

impl SettingKey {
    pub fn use_capability(&self) -> &'static str {
        match self {
            SettingKey::VoiceGuidanceEnabled => "accessibility:voiceguidance",
            SettingKey::ClosedCaptions => "accessibility:closedcaptions",
            SettingKey::AllowPersonalization => "discovery:policy",
            SettingKey::AllowWatchHistory => "discovery:policy",
            SettingKey::ShareWatchHistory => "discovery:policy",
            SettingKey::DeviceName => "device:name",
            SettingKey::PowerSaving => "",
            SettingKey::LegacyMiniGuide => "",
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SettingsRequestParam {
    pub context: CallContext,
    pub keys: Vec<SettingKey>,
    pub alias_map: Option<HashMap<String, String>>,
}

impl SettingsRequestParam {
    pub fn new(
        context: CallContext,
        keys: Vec<SettingKey>,
        alias_map: Option<HashMap<String, String>>,
    ) -> Self {
        Self {
            context,
            keys,
            alias_map,
        }
    }

    pub fn get_alias(&self, key: &SettingKey) -> String {
        if let Some(alias) = &self.alias_map {
            if let Some(s) = alias.get(&key.to_string()) {
                return s.to_owned();
            }
        }
        key.to_string()
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SettingsRequest {
    Get(SettingsRequestParam),
    Subscribe(SettingsRequestParam),
}

impl ExtnPayloadProvider for SettingsRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Settings(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Settings(value)) = payload {
            return Some(value);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Settings
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SettingValue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

impl SettingValue {
    pub fn string(value: String) -> SettingValue {
        SettingValue {
            value: Some(value),
            enabled: None,
        }
    }
    pub fn bool(enabled: bool) -> SettingValue {
        SettingValue {
            value: None,
            enabled: Some(enabled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::gateway::rpc_gateway_api::ApiProtocol;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_setting_key_to_string() {
        let setting_key = SettingKey::VoiceGuidanceEnabled;
        assert_eq!(setting_key.to_string(), "VoiceGuidanceEnabled");
    }

    #[test]
    fn test_setting_key_use_capability() {
        let setting_key = SettingKey::VoiceGuidanceEnabled;
        assert_eq!(setting_key.use_capability(), "accessibility:voiceguidance");
    }

    #[test]
    fn test_settings_request_param_get_alias() {
        let alias_map = Some(
            vec![
                ("VoiceGuidanceEnabled".to_owned(), "Alias1".to_owned()),
                ("ClosedCaptions".to_owned(), "Alias2".to_owned()),
            ]
            .into_iter()
            .collect(),
        );
        let settings_request_param = SettingsRequestParam::new(
            CallContext {
                session_id: "test_session_id".to_string(),
                request_id: "test_request_id".to_string(),
                app_id: "test_app_id".to_string(),
                call_id: 123,
                protocol: ApiProtocol::JsonRpc,
                method: "some method".to_string(),
                cid: Some("test_cid".to_string()),
                gateway_secure: true,
                context: Vec::new(),
            },
            vec![SettingKey::VoiceGuidanceEnabled, SettingKey::ClosedCaptions],
            alias_map,
        );
        assert_eq!(
            settings_request_param.get_alias(&SettingKey::VoiceGuidanceEnabled),
            "Alias1"
        );
        assert_eq!(
            settings_request_param.get_alias(&SettingKey::ClosedCaptions),
            "Alias2"
        );
    }

    #[test]
    fn test_extn_request_settings() {
        let settings_request_param = SettingsRequestParam {
            context: CallContext {
                session_id: "test_session_id".to_string(),
                request_id: "test_request_id".to_string(),
                app_id: "test_app_id".to_string(),
                call_id: 123,
                protocol: ApiProtocol::JsonRpc,
                method: "some method".to_string(),
                cid: Some("test_cid".to_string()),
                gateway_secure: true,
                context: Vec::new(),
            },
            keys: vec![SettingKey::VoiceGuidanceEnabled, SettingKey::ClosedCaptions],
            alias_map: Some(HashMap::new()),
        };

        let settings_request = SettingsRequest::Get(settings_request_param);

        let contract_type: RippleContract = RippleContract::Settings;
        test_extn_payload_provider(settings_request, contract_type);
    }
}
