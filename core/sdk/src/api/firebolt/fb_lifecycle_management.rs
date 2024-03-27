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
        device::entertainment_data::{InternalNavigationIntent, NavigationIntent},
    },
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

use super::{fb_discovery::LaunchRequest, fb_lifecycle::LifecycleState};

pub const LCM_EVENT_ON_REQUEST_READY: &str = "lifecyclemanagement.onRequestReady";
pub const LCM_EVENT_ON_REQUEST_CLOSE: &str = "lifecyclemanagement.onRequestClose";
pub const LCM_EVENT_ON_REQUEST_FINISHED: &str = "lifecyclemanagement.onRequestFinished";
pub const LCM_EVENT_ON_REQUEST_LAUNCH: &str = "lifecyclemanagement.onRequestLaunch";
pub const LCM_EVENT_ON_SESSION_TRANSITION_COMPLETED: &str =
    "lifecyclemanagement.onSessionTransitionCompleted";
pub const LCM_EVENT_ON_SESSION_TRANSITION_CANCELED: &str =
    "lifecyclemanagement.onSessionTransitionCanceled";

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct LifecycleManagementLaunchEvent {
    pub parameters: LifecycleManagementLaunchParameters,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementLaunchParameters {
    pub app_id: String,
    // Navigation Intent is untagged based on spec so we need the variant in the enum
    pub intent: Option<InternalNavigationIntent>,
}

impl LifecycleManagementLaunchParameters {
    fn get_intent(&self) -> Option<NavigationIntent> {
        if let Some(l) = self.intent.clone() {
            return Some(l.into());
        }
        None
    }

    pub fn get_launch_request(&self) -> LaunchRequest {
        LaunchRequest {
            app_id: self.app_id.clone(),
            intent: self.get_intent(),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct LifecycleManagementReadyEvent {
    pub parameters: LifecycleManagementReadyParameters,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementReadyParameters {
    pub app_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct LifecycleManagementCloseEvent {
    pub parameters: LifecycleManagementCloseParameters,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementCloseParameters {
    pub app_id: String,
    pub reason: CloseReason,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementFinishedEvent {
    pub parameters: LifecycleManagementFinishedParameters,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum LifecycleManagementProviderEvent {
    Add(String),
    Remove(String),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementFinishedParameters {
    pub app_id: String,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetStateRequest {
    pub app_id: String,
    pub state: LifecycleState,
}

#[derive(Serialize, PartialEq, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SessionResponse {
    Pending(PendingSessionResponse),
    Completed(CompletedSessionResponse),
}
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompletedSessionResponse {
    pub app_id: String,
    pub session_id: String,
    pub loaded_session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_session_id: Option<String>,
    pub transition_pending: bool,
}
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingSessionResponse {
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loaded_session_id: Option<String>,
    pub transition_pending: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AppSessionRequest {
    pub session: AppSession,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::apps::{
        AppBasicInfo, AppLaunchInfo, AppRuntime, AppRuntimeTransport, AppSession,
    };
    use crate::api::device::entertainment_data::{
        HomeIntent, InternalNavigationIntent, InternalNavigationIntentStrict,
        NavigationIntentLoose, NavigationIntentStrict,
    };
    use crate::api::firebolt::fb_discovery::DiscoveryContext;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_get_intent_some() {
        let launch_params = LifecycleManagementLaunchParameters {
            app_id: "test_app".to_string(),
            intent: Some(InternalNavigationIntent::NavigationIntentLoose(
                NavigationIntentLoose {
                    context: DiscoveryContext::new("voice"),
                    action: "test_action".to_string(),
                    data: None,
                },
            )),
        };

        let intent = launch_params.get_intent();
        assert_eq!(
            intent,
            Some(NavigationIntent::NavigationIntentLoose(
                NavigationIntentLoose {
                    context: DiscoveryContext::new("voice"),
                    action: "test_action".to_string(),
                    data: None,
                },
            ))
        );
    }

    #[test]
    fn test_get_intent_none() {
        let launch_params = LifecycleManagementLaunchParameters {
            app_id: "test_app".to_string(),
            intent: None,
        };

        let intent = launch_params.get_intent();
        assert_eq!(intent, None);
    }

    #[test]
    fn test_get_launch_request() {
        let launch_params = LifecycleManagementLaunchParameters {
            app_id: "test_app".to_string(),
            intent: Some(InternalNavigationIntent::NavigationIntentStrict(
                InternalNavigationIntentStrict::Home(HomeIntent {
                    context: DiscoveryContext::new("voice"),
                }),
            )),
        };

        let launch_request = launch_params.get_launch_request();
        assert_eq!(
            launch_request,
            LaunchRequest {
                app_id: "test_app".to_string(),
                intent: Some(NavigationIntent::NavigationIntentStrict(
                    NavigationIntentStrict::Home(HomeIntent {
                        context: DiscoveryContext::new("voice"),
                    }),
                )),
            }
        );
    }

    #[test]
    fn test_extn_request_lifecycle_management() {
        let app_session_request = AppSessionRequest {
            session: AppSession {
                app: AppBasicInfo {
                    id: "sample_id".to_string(),
                    catalog: None,
                    url: None,
                    title: None,
                },
                runtime: Some(AppRuntime {
                    id: Some("sample_runtime_id".to_string()),
                    transport: AppRuntimeTransport::Bridge,
                }),
                launch: AppLaunchInfo::default(),
            },
        };
        let lifecycle_management_request = LifecycleManagementRequest::Session(app_session_request);
        let contract_type: RippleContract = RippleContract::LifecycleManagement;
        test_extn_payload_provider(lifecycle_management_request, contract_type);
    }

    #[test]
    fn test_lifecycle_management_launch_event_serialization() {
        let launch_event =
            LifecycleManagementEventRequest::Launch(LifecycleManagementLaunchEvent {
                parameters: LifecycleManagementLaunchParameters {
                    app_id: "example_app".to_string(),
                    intent: Some(InternalNavigationIntent::NavigationIntentStrict(
                        InternalNavigationIntentStrict::Home(HomeIntent {
                            context: DiscoveryContext {
                                source: "test_source".to_string(),
                            },
                        }),
                    )),
                },
            });
        let contract_type: RippleContract = RippleContract::Launcher;
        test_extn_payload_provider(launch_event, contract_type);
    }
}
