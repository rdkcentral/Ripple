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
    api::session::{TokenType, SessionAdjective},
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenResult {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(rename = "type")]
    pub _type: TokenType,
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
