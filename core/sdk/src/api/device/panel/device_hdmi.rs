use super::device_av_input::AVInputAdjective;
use crate::api::device::device_request::DeviceRequest;
use crate::api::firebolt::panel::fb_hdmi::HdmiSelectOperationRequest;
use crate::extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest};
use crate::framework::ripple_contract::{ContractAdjective, RippleContract};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HdmiRequest {
    GetAvailableInputs,
    HdmiSelectOperation(HdmiSelectOperationRequest),
}

impl ContractAdjective for AVInputAdjective {
    fn get_contract(&self) -> RippleContract {
        RippleContract::AVInput(self.clone())
    }
}

impl ExtnPayloadProvider for HdmiRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Hdmi(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Hdmi(d))) = payload {
            return Some(d);
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::AVInput(AVInputAdjective::Hdmi)
    }
}
