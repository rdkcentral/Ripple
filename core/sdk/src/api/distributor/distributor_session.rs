use serde::{Deserialize, Serialize};

use crate::extn::{
    extn_capability::{ExtnCapability, ExtnClass},
    extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
};

use super::distributor_request::DistributorRequest;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DistributorSessionRequest {
    Session,
    IsProvisioned,
}

impl ExtnPayloadProvider for DistributorSessionRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(r) => match r {
                ExtnRequest::Distributor(v) => match v {
                    DistributorRequest::Session(s) => return Some(s),
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Distributor(DistributorRequest::Session(
            self.clone(),
        )))
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::new_extn(ExtnClass::Distributor, "session".into())
    }
}

#[repr(C)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DistributorSession {
    pub id: String,
    pub token: String,
    pub account_id: String,
    pub device_id: String,
}

impl ExtnPayloadProvider for DistributorSession {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Response(r) => match r {
                ExtnResponse::Session(v) => return Some(v),
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Session(self.clone()))
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::new_extn(ExtnClass::Distributor, "session".into())
    }
}

pub struct OptionalDistributorSession {
    pub id: Option<String>,
    pub token: Option<String>,
    pub account_id: Option<String>,
    pub device_id: Option<String>,
}

impl DistributorSession {
    pub fn get_only_id(&self) -> OptionalDistributorSession {
        OptionalDistributorSession {
            id: Some(self.id.clone()),
            token: None,
            account_id: None,
            device_id: None,
        }
    }
}
