use serde::{Serialize, Deserialize};

use crate::{extn::extn_client_message::{ExtnPayloadProvider, ExtnPayload, ExtnRequest, ExtnResponse}, framework::ripple_contract::RippleContract, api::gateway::rpc_gateway_api::CallContext};


pub const PIN_CHALLENGE_EVENT: &'static str = "pinchallenge.onRequestChallenge";
pub const PIN_CHALLENGE_CAPABILITY: &'static str = "xrn:firebolt:capability:usergrant:pinchallenge";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PinChallengeRequest {
    pub pin_space: PinSpace,
    pub requestor: CallContext,
    pub capability: Option<String>,
}

impl ExtnPayloadProvider for PinChallengeRequest {

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::PinChallenge(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::PinChallenge(r) => return Some(r),
                _ => {}
            },
            _ => {}
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
    granted: bool,
    reason: PinChallengeResultReason,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum PinSpace {
    Purchase,
    Content,
}


impl ExtnPayloadProvider for PinChallengeResponse {

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::PinChallenge(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Response(request) => match request {
                ExtnResponse::PinChallenge(r) => return Some(r),
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::PinChallenge
    }
}