use serde::{Deserialize, Serialize};

use crate::{
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider},
    framework::ripple_contract::{MainContract, RippleContract},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExtnStatus {
    Ready,
    Interrupted,
}

impl ExtnPayloadProvider for ExtnStatus {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::Status(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<ExtnStatus> {
        match payload {
            ExtnPayload::Event(request) => match request {
                ExtnEvent::Status(r) => return Some(r),
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::Main(MainContract::ExtnStatus)
    }
}
