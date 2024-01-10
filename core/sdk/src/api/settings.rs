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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
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
