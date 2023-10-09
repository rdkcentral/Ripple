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

use super::provider::ChallengeRequestor;

pub const PIN_CHALLENGE_EVENT: &str = "pinchallenge.onRequestChallenge";
pub const PIN_CHALLENGE_CAPABILITY: &str = "xrn:firebolt:capability:usergrant:pinchallenge";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PinChallengeRequest {
    pub pin_space: PinSpace,
    pub requestor: ChallengeRequestor,
    pub capability: Option<String>,
}
impl From<PinChallengeRequestWithContext> for PinChallengeRequest {
    fn from(pin_req: PinChallengeRequestWithContext) -> Self {
        PinChallengeRequest {
            pin_space: pin_req.pin_space,
            requestor: pin_req.requestor.to_owned(),
            capability: pin_req.capability,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PinChallengeRequestWithContext {
    pub pin_space: PinSpace,
    pub requestor: ChallengeRequestor,
    pub capability: Option<String>,
    pub call_ctx: CallContext,
}

impl ExtnPayloadProvider for PinChallengeRequestWithContext {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PinChallenge(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::PinChallenge(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::PinChallenge
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PinChallengeResultReason {
    NoPinRequired,
    NoPinRequiredWindow,
    ExceededPinFailures,
    CorrectPin,
    Cancelled,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PinChallengeResponse {
    pub granted: Option<bool>,
    pub reason: PinChallengeResultReason,
}
impl PinChallengeResponse {
    pub fn get_granted(&self) -> Option<bool> {
        self.granted
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum PinSpace {
    Purchase,
    Content,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PinChallengeConfiguration {
    pub pin_space: PinSpace,
}

impl ExtnPayloadProvider for PinChallengeResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::PinChallenge(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::PinChallenge(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::PinChallenge
    }
}
