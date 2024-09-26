use serde::{Deserialize, Serialize};

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

use super::device_request::DeviceRequest;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum UserSettingsRequest {
    GetAudioDescription,
    SetAudioDescription(bool),
    GetPreferredAudioLanguages,
    SetPreferredAudioLanguages,
    GetPresentationLanguage,
    SetPresentationLanguage,
    GetCaptions,
    SetCaptions,
    GetPreferredCaptionsLanguages,
    SetPreferredCaptionsLanguages,
    GetPreferredClosedCaptionService,
    SetPreferredClosedCaptionService,
    GetPrivacyMode,
    SetPrivacyMode,
}

impl ExtnPayloadProvider for UserSettingsRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::UserSettings(
            self.clone(),
        )))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::UserSettings(request))) =
            payload
        {
            return Some(request);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Settings
    }
}
