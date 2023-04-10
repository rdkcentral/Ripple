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

use serde::{Deserialize, Serialize};

use super::provider::{ProviderResponse, ProviderResponsePayload};

pub const EMAIL_EVENT_PREFIX: &'static str = "keyboard.onRequestEmail";
pub const PASSWORD_EVENT_PREFIX: &'static str = "keyboard.onRequestPassword";
pub const STANDARD_EVENT_PREFIX: &'static str = "keyboard.onRequestStandard";

pub const KEYBOARD_PROVIDER_CAPABILITY: &'static str = "xrn:firebolt:capability:input:keyboard";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum KeyboardType {
    Email,
    Password,
    Standard,
}

impl KeyboardType {
    pub fn to_provider_method(&self) -> &str {
        match self {
            KeyboardType::Email => "email",
            KeyboardType::Password => "password",
            KeyboardType::Standard => "standard",
        }
    }
}

#[derive(Deserialize)]
pub struct KeyboardRequestPassword {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct KeyboardRequestEmail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(rename = "type")]
    pub _type: EmailUsage,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmailUsage {
    SignIn,
    SignUp,
}

#[derive(Deserialize)]
pub struct KeyboardRequest {
    pub message: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardProviderResponse {
    correlation_id: String,
    result: KeyboardResult,
}

impl KeyboardProviderResponse {
    pub fn to_provider_response(&self) -> ProviderResponse {
        ProviderResponse {
            correlation_id: self.correlation_id.clone(),
            result: ProviderResponsePayload::KeyboardResult(self.result.clone()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KeyboardSession {
    #[serde(rename = "type")]
    pub _type: KeyboardType,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KeyboardResult {
    pub text: String,
    pub canceled: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PromptEmailRequest {
    pub prefill_type: PrefillType,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PrefillType {
    SignIn,
    SignUp,
}
