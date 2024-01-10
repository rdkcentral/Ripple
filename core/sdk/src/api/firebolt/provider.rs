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

use crate::api::device::entertainment_data::{
    EntityInfoParameters, EntityInfoResult, PurchasedContentParameters, PurchasedContentResult,
};

use super::{
    fb_keyboard::{KeyboardSessionRequest, KeyboardSessionResponse},
    fb_pin::{PinChallengeRequest, PinChallengeResponse},
};

pub const ACK_CHALLENGE_EVENT: &str = "acknowledgechallenge.onRequestChallenge";
pub const ACK_CHALLENGE_CAPABILITY: &str = "xrn:firebolt:capability:usergrant:acknowledgechallenge";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ProviderRequestPayload {
    KeyboardSession(KeyboardSessionRequest),
    PinChallenge(PinChallengeRequest),
    AckChallenge(Challenge),
    EntityInfoRequest(EntityInfoParameters),
    PurchasedContentRequest(PurchasedContentParameters),
    Generic(String),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ProviderResponsePayload {
    ChallengeResponse(ChallengeResponse),
    ChallengeError(ChallengeError),
    PinChallengeResponse(PinChallengeResponse),
    KeyboardResult(KeyboardSessionResponse),
    EntityInfoResponse(Option<EntityInfoResult>),
    PurchasedContentResponse(PurchasedContentResult),
}

impl ProviderResponsePayload {
    pub fn as_keyboard_result(&self) -> Option<KeyboardSessionResponse> {
        match self {
            ProviderResponsePayload::KeyboardResult(res) => Some(res.clone()),
            _ => None,
        }
    }

    pub fn as_pin_challenge_response(&self) -> Option<PinChallengeResponse> {
        match self {
            ProviderResponsePayload::PinChallengeResponse(res) => Some(res.clone()),
            _ => None,
        }
    }

    pub fn as_challenge_response(&self) -> Option<ChallengeResponse> {
        match self {
            ProviderResponsePayload::ChallengeResponse(res) => {
                res.granted.map(|value| ChallengeResponse {
                    granted: Some(value),
                })
            }
            ProviderResponsePayload::PinChallengeResponse(res) => {
                res.get_granted().map(|value| ChallengeResponse {
                    granted: Some(value),
                })
            }
            _ => None,
        }
    }

    pub fn as_entity_info_result(&self) -> Option<Option<EntityInfoResult>> {
        match self {
            ProviderResponsePayload::EntityInfoResponse(res) => Some(res.clone()),
            _ => None,
        }
    }

    pub fn as_purchased_content_result(&self) -> Option<PurchasedContentResult> {
        match self {
            ProviderResponsePayload::PurchasedContentResponse(res) => Some(res.clone()),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRequest {
    pub correlation_id: String,
    pub parameters: ProviderRequestPayload,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProviderResponse {
    pub correlation_id: String,
    pub result: ProviderResponsePayload,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProviderRequest<T> {
    pub correlation_id: String,
    pub parameters: T,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProviderResponse<T> {
    pub correlation_id: String,
    pub result: T,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChallengeResponse {
    pub granted: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DataObject {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChallengeError {
    pub code: u32,
    pub message: String,
    pub data: Option<DataObject>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChallengeRequestor {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FocusRequest {
    pub correlation_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Challenge {
    pub capability: String,
    pub requestor: ChallengeRequestor,
}
