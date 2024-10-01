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
    SetPreferredAudioLanguages(Vec<String>),
    GetPresentationLanguage,
    SetPresentationLanguage,
    GetClosedCaptionsEnabled,
    SetClosedCaptionsEnabled(bool),
    GetPreferredCaptionsLanguages,
    SetPreferredCaptionsLanguages(Vec<String>),
    GetPreferredClosedCaptionService,
    SetPreferredClosedCaptionService(String),
    GetPrivacyMode,
    SetPrivacyMode(String),
    GetClosedCaptionsFontFamily,
    SetClosedCaptionsFontFamily(String),
    GetClosedCaptionsFontSize,
    SetClosedCaptionsFontSize(String),
    GetClosedCaptionsFontColor,
    SetClosedCaptionsFontColor(String),
    GetClosedCaptionsFontOpacity,
    SetClosedCaptionsFontOpacity(String),
    GetClosedCaptionsFontEdge,
    SetClosedCaptionsFontEdge(String),
    GetClosedCaptionsFontEdgeColor,
    SetClosedCaptionsFontEdgeColor(String),
    GetClosedCaptionsBackgroundColor,
    SetClosedCaptionsBackgroundColor(String),
    GetClosedCaptionsBackgroundOpacity,
    SetClosedCaptionsBackgroundOpacity(String),
    GetClosedCaptionsWindowColor,
    SetClosedCaptionsWindowColor(String),
    GetClosedCaptionsWindowOpacity,
    SetClosedCaptionsWindowOpacity(String),
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
