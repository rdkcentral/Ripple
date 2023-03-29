use serde::{Deserialize, Serialize};

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SessionRequest {
    Get,
    IsProvisioned,
}

impl ExtnPayloadProvider for SessionRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(r) => match r {
                ExtnRequest::Session(v) => return Some(v),
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Session(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::AccountSession
    }
}

#[repr(C)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountSession {
    pub id: String,
    pub token: String,
    pub account_id: String,
    pub device_id: String,
}

impl ExtnPayloadProvider for AccountSession {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Response(r) => match r {
                ExtnResponse::AccountSession(v) => return Some(v),
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::AccountSession(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::AccountSession
    }
}

pub struct OptionalRippleSession {
    pub id: Option<String>,
    pub token: Option<String>,
    pub account_id: Option<String>,
    pub device_id: Option<String>,
}

impl AccountSession {
    pub fn get_only_id(&self) -> OptionalRippleSession {
        OptionalRippleSession {
            id: Some(self.id.clone()),
            token: None,
            account_id: None,
            device_id: None,
        }
    }
}
