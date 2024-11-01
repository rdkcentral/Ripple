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

use crate::{
    api::gateway::rpc_gateway_api::CallContext,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

use super::provider::{ProviderResponse, ProviderResponsePayload};

pub const EMAIL_EVENT_PREFIX: &str = "keyboard.onRequestEmail";
pub const PASSWORD_EVENT_PREFIX: &str = "keyboard.onRequestPassword";
pub const STANDARD_EVENT_PREFIX: &str = "keyboard.onRequestStandard";

pub const KEYBOARD_PROVIDER_CAPABILITY: &str = "xrn:firebolt:capability:input:keyboard";

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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
    result: KeyboardSessionResponse,
}

impl KeyboardProviderResponse {
    pub fn to_provider_response(&self) -> ProviderResponse {
        ProviderResponse {
            correlation_id: self.correlation_id.clone(),
            result: ProviderResponsePayload::KeyboardResult(self.result.clone()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct KeyboardSessionRequest {
    #[serde(rename = "type")]
    pub _type: KeyboardType,
    pub ctx: CallContext,
    pub message: String,
}

impl ExtnPayloadProvider for KeyboardSessionRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Keyboard(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Keyboard(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Keyboard
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct KeyboardSessionResponse {
    pub text: String,
    pub canceled: bool,
}

impl ExtnPayloadProvider for KeyboardSessionResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Keyboard(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::Keyboard(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Keyboard
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::gateway::rpc_gateway_api::{ApiProtocol, CallContext};
    use crate::utils::test_utils::test_extn_payload_provider;
    #[test]
    fn test_keyboard_type_to_provider_method() {
        assert_eq!(KeyboardType::Email.to_provider_method(), "email");
        assert_eq!(KeyboardType::Password.to_provider_method(), "password");
        assert_eq!(KeyboardType::Standard.to_provider_method(), "standard");
    }

    #[test]
    fn test_to_provider_response() {
        let keyboard_response = KeyboardProviderResponse {
            correlation_id: "123456".to_string(),
            result: KeyboardSessionResponse {
                text: "Test response text".to_string(),
                canceled: false,
            },
        };

        let provider_response = keyboard_response.to_provider_response();

        assert_eq!(provider_response.correlation_id, "123456");
        assert_eq!(
            provider_response.result,
            ProviderResponsePayload::KeyboardResult(KeyboardSessionResponse {
                text: "Test response text".to_string(),
                canceled: false
            })
        );
    }

    #[test]
    fn test_extn_request_keyboard_session() {
        let keyboard_session_request = KeyboardSessionRequest {
            _type: KeyboardType::Email,
            ctx: CallContext {
                session_id: "test_session_id".to_string(),
                request_id: "test_request_id".to_string(),
                app_id: "test_app_id".to_string(),
                call_id: 123,
                protocol: ApiProtocol::Extn,
                method: "POST".to_string(),
                cid: Some("test_cid".to_string()),
                gateway_secure: true,
            },
            message: "test_message".to_string(),
        };
        let contract_type: RippleContract = RippleContract::Keyboard;
        test_extn_payload_provider(keyboard_session_request, contract_type);
    }

    #[test]
    fn test_extn_response_keyboard_session() {
        let keyboard_session_response = KeyboardSessionResponse {
            text: "Test response text".to_string(),
            canceled: false,
        };

        let contract_type: RippleContract = RippleContract::Keyboard;
        test_extn_payload_provider(keyboard_session_response, contract_type);
    }
}
