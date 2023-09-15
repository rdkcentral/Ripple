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
    api::session::AccountSession,
    //api::{firebolt::fb_authentication::TokenResult, session::AccountSession},
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlatformTokenRequest {
    pub options: Vec<String>,
    pub context: PlatformTokenContext,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlatformTokenContext {
    pub app_id: String,
    pub content_provider: String,
    pub device_session_id: String,
    pub app_session_id: String,
    pub dist_session: AccountSession,
}

impl ExtnPayloadProvider for PlatformTokenRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PlatformToken(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<PlatformTokenRequest> {
        if let ExtnPayload::Request(ExtnRequest::PlatformToken(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::PlatformSessionToken
    }
}

// pub type PlatformTokenResponse = TokenResult;

// impl ExtnPayloadProvider for PlatformTokenResponse {
//     fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
//         if let ExtnPayload::Response(ExtnResponse::Token(v)) = payload {
//             return Some(v);
//         }

//         None
//     }

//     fn get_extn_payload(&self) -> ExtnPayload {
//         ExtnPayload::Response(ExtnResponse::Token(self.clone()))
//     }

//     fn contract() -> crate::framework::ripple_contract::RippleContract {
//         RippleContract::PlatformSessionToken
//     }
// }
