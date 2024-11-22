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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PinChallengeResultReason {
    NoPinRequired,
    NoPinRequiredWindow,
    ExceededPinFailures,
    CorrectPin,
    Cancelled,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PinChallengeResponse {
    pub granted: Option<bool>,
    pub reason: PinChallengeResultReason,
}
impl PinChallengeResponse {
    pub fn get_granted(&self) -> Option<bool> {
        self.granted
    }
    pub fn get_reason(&self) -> PinChallengeResultReason {
        self.reason.clone()
    }
    pub fn new(granted: Option<bool>, reason: PinChallengeResultReason) -> Self {
        PinChallengeResponse { granted, reason }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::gateway::rpc_gateway_api::{ApiProtocol, CallContext};
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_pin_challenge_request_from_context() {
        let pin_space = PinSpace::Purchase;
        let requestor = ChallengeRequestor {
            id: "id".to_string(),
            name: "name".to_string(),
        };
        let capability = Some(String::from("capability"));

        let context = CallContext {
            session_id: "session_id".to_string(),
            request_id: "request_id".to_string(),
            app_id: "app_id".to_string(),
            call_id: 123,
            protocol: ApiProtocol::Bridge,
            method: "POST".to_string(),
            cid: Some("cid".to_string()),
            gateway_secure: true,
            context: Vec::new(),
        };

        let pin_challenge_request_with_context = PinChallengeRequestWithContext {
            pin_space: pin_space.clone(),
            requestor: requestor.clone(),
            capability: capability.clone(),
            call_ctx: context,
        };

        let pin_challenge_request = PinChallengeRequest::from(pin_challenge_request_with_context);

        assert_eq!(pin_challenge_request.pin_space, pin_space);
        assert_eq!(pin_challenge_request.requestor, requestor);
        assert_eq!(pin_challenge_request.capability, capability);
    }

    #[test]
    fn test_pin_challenge_response_getters() {
        let granted = Some(true);
        let reason = PinChallengeResultReason::CorrectPin;
        let pin_challenge_response = PinChallengeResponse::new(granted, reason.clone());

        assert_eq!(pin_challenge_response.get_granted(), granted);
        assert_eq!(pin_challenge_response.get_reason(), reason);
    }

    #[test]
    fn test_extn_request_pin_challenge_with_context() {
        let pin_challenge_request = PinChallengeRequestWithContext {
            pin_space: PinSpace::Purchase,
            requestor: ChallengeRequestor {
                id: "test_requestor_id".to_string(),
                name: "eest_requestor_name".to_string(),
            },
            capability: Some("test_capability".to_string()),
            call_ctx: CallContext {
                session_id: "test_session_id".to_string(),
                request_id: "test_request_id".to_string(),
                app_id: "test_app_id".to_string(),
                call_id: 123,
                protocol: ApiProtocol::Bridge,
                method: "POST".to_string(),
                cid: Some("test_cid".to_string()),
                gateway_secure: true,
                context: Vec::new(),
            },
        };
        let contract_type: RippleContract = RippleContract::PinChallenge;
        test_extn_payload_provider(pin_challenge_request, contract_type);
    }

    #[test]
    fn test_extn_response_pin_challenge() {
        let pin_challenge_response = PinChallengeResponse {
            granted: Some(true),
            reason: PinChallengeResultReason::NoPinRequired,
        };

        let contract_type: RippleContract = RippleContract::PinChallenge;
        test_extn_payload_provider(pin_challenge_response, contract_type);
    }
}
