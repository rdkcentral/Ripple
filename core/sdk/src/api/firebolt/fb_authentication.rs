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
    api::session::{SessionAdjective, TokenType},
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenResult {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(rename = "type")]
    pub _type: TokenType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
}

impl std::fmt::Debug for TokenResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResult")
            .field("expires", &self.expires)
            .field("_type", &self._type)
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .field("token_type", &self.token_type)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TokenRequest {
    #[serde(rename = "type")]
    pub _type: TokenType,
}

impl ExtnPayloadProvider for TokenResult {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Token(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<TokenResult> {
        if let ExtnPayload::Response(ExtnResponse::Token(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Session(SessionAdjective::Device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;
    use rstest::rstest;

    #[rstest]
    #[case(None, TokenType::Device)]
    #[case(Some("2024-12-31".to_string()), TokenType::Platform)]
    fn test_fmt_token_result(#[case] expires_value: Option<String>, #[case] token_type: TokenType) {
        let token_result = TokenResult {
            value: "some_value".to_string(),
            expires: expires_value.clone(),
            _type: token_type,
            scope: Some("some_scope".to_string()),
            expires_in: Some(3600),
            token_type: Some("Bearer".to_string()),
        };

        // Define expected JSON structure using serde_json::json! macro
        let mut expected_json = serde_json::json!({
            "value": "some_value",
            "scope": "some_scope",
            "expiresIn": 3600,
            "tokenType": "Bearer",
            "type": match token_type {
                TokenType::Device => "device",
                TokenType::Platform => "platform",
                TokenType::Root => "root",
                TokenType::Distributor => "distributor",
            },
        });

        // Conditionally add expires field to the expected JSON object if expires_value is Some
        if let Some(expires) = expires_value {
            expected_json["expires"] = serde_json::Value::String(expires);
        }

        // Serialize the actual TokenResult object into a JSON string
        let actual_json = serde_json::to_value(token_result).unwrap();

        // Assert that both JSON objects are equal regardless of the order
        assert_eq!(expected_json, actual_json);
    }

    #[test]
    fn test_token_result() {
        let token_result = TokenResult {
            value: "test_value".to_string(),
            expires: Some("2024-12-31T23:59:59Z".to_string()),
            _type: TokenType::Platform,
            scope: Some("test_scope".to_string()),
            expires_in: Some(3600),
            token_type: Some("Bearer".to_string()),
        };

        let contract_type: RippleContract = RippleContract::Session(SessionAdjective::Device);
        test_extn_payload_provider(token_result, contract_type);
    }
}
