use serde::{Deserialize, Serialize};

use crate::{
    api::apps::{AppSession, CloseReason},
    extn::{
        extn_capability::{ExtnCapability, ExtnClass},
        extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    },
};

use super::{fb_discovery::NavigationIntent, fb_lifecycle::LifecycleState};

pub const LCM_EVENT_ON_REQUEST_READY: &'static str = "lifecyclemanagement.onRequestReady";
pub const LCM_EVENT_ON_REQUEST_CLOSE: &'static str = "lifecyclemanagement.onRequestClose";
pub const LCM_EVENT_ON_REQUEST_FINISHED: &'static str = "lifecyclemanagement.onRequestFinished";
pub const LCM_EVENT_ON_REQUEST_LAUNCH: &'static str = "lifecyclemanagement.onRequestLaunch";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum LifecycleManagementEventRequest {
    Launch(LifecycleManagementLaunchEvent),
    Ready(LifecycleManagementReadyEvent),
    Close(LifecycleManagementCloseEvent),
    Finished(LifecycleManagementFinishedEvent),
}

impl ExtnPayloadProvider for LifecycleManagementEventRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::Value(serde_json::to_value(self).unwrap()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Event(event) => match event {
                ExtnEvent::Value(value) => {
                    let result: Result<Self, serde_json::Error> = serde_json::from_value(value);
                    if result.is_ok() {
                        return Some(result.unwrap());
                    }
                }
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::new_channel(ExtnClass::Launcher, "launcher".into())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum LifecycleManagementRequest {
    Session(AppSessionRequest),
    SetState(SetStateRequest),
}

impl ExtnPayloadProvider for LifecycleManagementRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::LifecycleManagement(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(event) => match event {
                ExtnRequest::LifecycleManagement(value) => return Some(value),
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::get_main_target("lifecycle-management".into())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LifecycleManagementLaunchEvent {
    pub parameters: LifecycleManagementLaunchParameters,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementLaunchParameters {
    pub app_id: String,
    pub intent: Option<NavigationIntent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LifecycleManagementReadyEvent {
    pub parameters: LifecycleManagementReadyParameters,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementReadyParameters {
    pub app_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LifecycleManagementCloseEvent {
    pub parameters: LifecycleManagementCloseParameters,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementCloseParameters {
    pub app_id: String,
    pub reason: CloseReason,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementFinishedEvent {
    pub parameters: LifecycleManagementFinishedParameters,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementFinishedParameters {
    pub app_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetStateRequest {
    pub app_id: String,
    pub state: LifecycleState,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSessionRequest {
    pub session: AppSession,
}
