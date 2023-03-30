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
use serde::{Serialize, Deserialize};

use super::{fb_keyboard::{KeyboardResult, KeyboardSession}, fb_pin::{PinChallengeRequest, PinChallengeResponse}};

pub const CHALLENGE_EVENT: &'static str = "acknowledgechallenge.onRequestChallenge";
pub const ACK_CHALLENGE_CAPABILITY: &'static str =
    "xrn:firebolt:capability:usergrant:acknowledgechallenge";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ProviderRequestPayload {
    KeyboardSession(KeyboardSession),
    PinChallenge(PinChallengeRequest),
    Generic(String),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ProviderResponsePayload {
    ChallengeResponse(ChallengeResponse),
    PinChallengeResponse(PinChallengeResponse),
    KeyboardResult(KeyboardResult)
}

impl ProviderResponsePayload {
    pub fn as_keyboard_result(&self) -> Option<KeyboardResult> {
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
    pub granted: bool,
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