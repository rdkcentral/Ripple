// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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

use serde::{Deserialize, Serialize};

use crate::{
    api::apps::{AppSession, CloseReason},
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
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

    fn contract() -> RippleContract {
        RippleContract::Launcher
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

    fn contract() -> RippleContract {
        RippleContract::LifecycleManagement
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
