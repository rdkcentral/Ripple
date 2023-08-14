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
    api::{
        apps::{AppSession, CloseReason},
        device::entertainment_data::NavigationIntent,
    },
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

use super::fb_lifecycle::LifecycleState;

pub const LCM_EVENT_ON_REQUEST_READY: &str = "lifecyclemanagement.onRequestReady";
pub const LCM_EVENT_ON_REQUEST_CLOSE: &str = "lifecyclemanagement.onRequestClose";
pub const LCM_EVENT_ON_REQUEST_FINISHED: &str = "lifecyclemanagement.onRequestFinished";
pub const LCM_EVENT_ON_REQUEST_LAUNCH: &str = "lifecyclemanagement.onRequestLaunch";
pub const LCM_EVENT_ON_SESSION_TRANSITION_COMPLETED: &str =
    "lifecyclemanagement.onSessionTransitionCompleted";
pub const LCM_EVENT_ON_SESSION_TRANSITION_CANCELED: &str =
    "lifecyclemanagement.onSessionTransitionCanceled";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum LifecycleManagementEventRequest {
    Launch(LifecycleManagementLaunchEvent),
    Ready(LifecycleManagementReadyEvent),
    Close(LifecycleManagementCloseEvent),
    Finished(LifecycleManagementFinishedEvent),
    Provide(LifecycleManagementProviderEvent),
}

impl ExtnPayloadProvider for LifecycleManagementEventRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::Value(serde_json::to_value(self).unwrap()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Event(ExtnEvent::Value(value)) = payload {
            let result: Result<Self, serde_json::Error> = serde_json::from_value(value);
            if let Ok(result) = result {
                return Some(result);
            }
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
    Close(String, CloseReason),
    Ready(String),
    GetSecondScreenPayload(String),
    StartPage(String),
}

impl ExtnPayloadProvider for LifecycleManagementRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::LifecycleManagement(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::LifecycleManagement(value)) = payload {
            return Some(value);
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
pub enum LifecycleManagementProviderEvent {
    Add(String),
    Remove(String),
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SessionResponse {
    Pending(PendingSessionResponse),
    Completed(CompletedSessionResponse),
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompletedSessionResponse {
    pub app_id: String,
    pub session_id: String,
    pub loaded_session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_session_id: Option<String>,
    pub transition_pending: bool,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PendingSessionResponse {
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loaded_session_id: Option<String>,
    pub transition_pending: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSessionRequest {
    pub session: AppSession,
}
