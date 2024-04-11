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

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::{
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    framework::ripple_contract::RippleContract,
    utils::{channel_utils::oneshot_send_and_log, error::RippleError},
};

use super::{
    device::entertainment_data::NavigationIntent,
    firebolt::{
        fb_discovery::LaunchRequest, fb_general::ListenRequest, fb_lifecycle::LifecycleState,
        fb_lifecycle_management::SessionResponse, fb_parameters::SecondScreenEvent,
    },
    gateway::rpc_gateway_api::CallContext,
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct AppSession {
    pub app: AppBasicInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<AppRuntime>,
    #[serde(default)]
    pub launch: AppLaunchInfo,
}

impl AppSession {
    // Gets the actual transport that will be used based on fallbacks
    // If no runtime or runtime.id is given, use Websocket
    // Otherwise use the transport given in the runtime
    pub fn get_transport(&self) -> EffectiveTransport {
        match &self.runtime {
            Some(rt) => match rt.transport {
                AppRuntimeTransport::Bridge => match &rt.id {
                    Some(id) => EffectiveTransport::Bridge(id.clone()),
                    None => EffectiveTransport::Websocket,
                },
                AppRuntimeTransport::Websocket => EffectiveTransport::Websocket,
            },
            None => EffectiveTransport::Websocket,
        }
    }

    pub fn update_intent(&mut self, intent: NavigationIntent) {
        let _ = self.launch.intent.insert(intent);
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct AppBasicInfo {
    pub id: String,
    pub catalog: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum AppRuntimeTransport {
    Bridge,
    Websocket,
}

fn runtime_transport_default() -> AppRuntimeTransport {
    AppRuntimeTransport::Websocket
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AppRuntime {
    pub id: Option<String>,
    #[serde(default = "runtime_transport_default")]
    pub transport: AppRuntimeTransport,
}

#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone)]
pub struct AppLaunchInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<NavigationIntent>,
    #[serde(
        default,
        rename = "secondScreen",
        skip_serializing_if = "Option::is_none"
    )]
    pub second_screen: Option<SecondScreenEvent>,
    #[serde(default = "bool::default")]
    pub inactive: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub enum EffectiveTransport {
    Bridge(String),
    Websocket,
}

impl std::fmt::Display for EffectiveTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            EffectiveTransport::Bridge(id) => write!(f, "bridge_{}", id),
            EffectiveTransport::Websocket => write!(f, "websocket"),
        }
    }
}

pub type AppResponse = Result<AppManagerResponse, AppError>;

impl ExtnPayloadProvider for AppResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        let response = if self.is_ok() {
            if let Ok(resp) = self {
                ExtnResponse::Value(serde_json::to_value(resp.clone()).unwrap())
            } else {
                ExtnResponse::None(())
            }
        } else {
            ExtnResponse::Error(RippleError::ProcessorError)
        };

        ExtnPayload::Response(response)
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(resp) = payload {
            match resp {
                ExtnResponse::Value(v) => {
                    if let Ok(v) = serde_json::from_value(v) {
                        return Some(Ok(v));
                    }
                }
                ExtnResponse::Error(_) => return Some(Err(AppError::General)),
                _ => {}
            }
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::LifecycleManagement
    }
}

#[derive(Debug, Clone)]
pub struct AppRequest {
    pub method: AppMethod,
    pub resp_tx: Arc<RwLock<Option<oneshot::Sender<AppResponse>>>>, // Allow fire-and-forget.
}

impl AppRequest {
    pub fn new(method: AppMethod, sender: oneshot::Sender<AppResponse>) -> AppRequest {
        AppRequest {
            method,
            resp_tx: Arc::new(RwLock::new(Some(sender))),
        }
    }

    pub fn send_response(&self, response: AppResponse) -> Result<(), RippleError> {
        let mut sender = self.resp_tx.write().unwrap();
        if sender.is_some() {
            oneshot_send_and_log(sender.take().unwrap(), response, "AppManager response");
            Ok(())
        } else {
            Err(RippleError::SenderMissing)
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum AppManagerResponse {
    None,
    State(LifecycleState),
    WaitingForReady,
    ViewId(Uuid),
    AppContentCatalog(Option<String>),
    StartPage(Option<String>),
    LaunchRequest(LaunchRequest),
    SessionId(String),
    SecondScreenPayload(String),
    AppName(Option<String>),
    Session(SessionResponse),
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq)]
pub enum AppError {
    General,
    NotFound,
    IoError,
    OsError,
    NotSupported,
    UnexpectedState,
    Timeout,
    Pending,
    AppNotReady,
    NoIntentError,
}

#[derive(Debug, Clone)]
pub enum AppMethod {
    Launch(LaunchRequest),
    Ready(String),
    State(String),
    Close(String, CloseReason),
    Finished(String),
    CheckReady(String, u128),
    CheckFinished(String),
    GetAppContentCatalog(String),
    GetViewId(String),
    GetStartPage(String),
    GetLaunchRequest(String),
    SetState(String, LifecycleState),
    BrowserSession(AppSession),
    GetSecondScreenPayload(String),
    GetAppName(String),
    NewActiveSession(AppSession),
    NewLoadedSession(AppSession),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum CloseReason {
    RemoteButton,
    UserExit,
    Error,
    AppNotReady,
    ResourceContention,
    Done,
}

impl CloseReason {
    pub fn as_string(&self) -> &'static str {
        match self {
            CloseReason::RemoteButton => "remoteButton",
            CloseReason::UserExit => "userExit",
            CloseReason::Error => "error",
            CloseReason::AppNotReady => "appNotReady",
            CloseReason::ResourceContention => "resourceContention",
            CloseReason::Done => "done",
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct StateChange {
    pub previous: LifecycleState,
    pub state: LifecycleState,
}
pub type ViewId = Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Dimensions {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AppEvent {
    pub event_name: String,
    pub result: Value,
    pub context: Option<Value>,
    pub app_id: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AppEventRequest {
    Emit(AppEvent),
    Register(CallContext, String, ListenRequest),
}

impl ExtnPayloadProvider for AppEventRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Event(ExtnEvent::Value(v)) = payload {
            if let Ok(v) = serde_json::from_value(v) {
                return Some(v);
            }
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::AppEvents
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::{
            device::entertainment_data::{HomeIntent, NavigationIntentStrict},
            firebolt::fb_discovery::DiscoveryContext,
        },
        utils::test_utils::test_extn_payload_provider,
    };

    #[test]
    fn test_get_transport_with_bridge_runtime_and_id() {
        let app = AppSession {
            app: AppBasicInfo {
                id: "app_id".to_string(),
                catalog: None,
                url: None,
                title: None,
            },
            runtime: Some(AppRuntime {
                id: Some("runtime_id".to_string()),
                transport: AppRuntimeTransport::Bridge,
            }),
            launch: AppLaunchInfo::default(),
        };

        assert_eq!(
            app.get_transport(),
            EffectiveTransport::Bridge("runtime_id".to_string())
        );
    }

    #[test]
    fn test_get_transport_with_bridge_runtime_and_no_id() {
        let app = AppSession {
            app: AppBasicInfo {
                id: "app_id".to_string(),
                catalog: None,
                url: None,
                title: None,
            },
            runtime: Some(AppRuntime {
                id: None,
                transport: AppRuntimeTransport::Bridge,
            }),
            launch: AppLaunchInfo::default(),
        };

        assert_eq!(app.get_transport(), EffectiveTransport::Websocket);
    }

    #[test]
    fn test_get_transport_with_websocket_runtime() {
        let app = AppSession {
            app: AppBasicInfo {
                id: "app_id".to_string(),
                catalog: None,
                url: None,
                title: None,
            },
            runtime: Some(AppRuntime {
                id: Some("runtime_id".to_string()),
                transport: AppRuntimeTransport::Websocket,
            }),
            launch: AppLaunchInfo::default(),
        };

        assert_eq!(app.get_transport(), EffectiveTransport::Websocket);
    }

    #[test]
    fn test_get_transport_with_no_runtime() {
        let app = AppSession {
            app: AppBasicInfo {
                id: "app_id".to_string(),
                catalog: None,
                url: None,
                title: None,
            },
            runtime: None,
            launch: AppLaunchInfo::default(),
        };

        assert_eq!(app.get_transport(), EffectiveTransport::Websocket);
    }

    #[test]
    fn test_update_intent() {
        let mut app = AppSession {
            app: AppBasicInfo {
                id: "app_id".to_string(),
                catalog: None,
                url: None,
                title: None,
            },
            runtime: None,
            launch: AppLaunchInfo::default(),
        };

        let home_intent = HomeIntent {
            context: DiscoveryContext {
                source: "test_source".to_string(),
            },
        };

        app.update_intent(NavigationIntent::NavigationIntentStrict(
            NavigationIntentStrict::Home(home_intent.clone()),
        ));

        assert_eq!(
            app.launch.intent,
            Some(NavigationIntent::NavigationIntentStrict(
                NavigationIntentStrict::Home(home_intent),
            ))
        );
    }

    #[tokio::test]
    async fn test_send_response_success() {
        // Create a mock response
        let response = Ok(AppManagerResponse::None);

        // Create a mock sender
        let (sender, receiver) = oneshot::channel();

        // Create an instance of AppRequest with the mock sender
        let app_request = AppRequest {
            method: AppMethod::Ready(String::from("ready")),
            resp_tx: Arc::new(RwLock::new(Some(sender))),
        };

        // Call the send_response function
        let result = app_request.send_response(response.clone());

        // Assert that the result is Ok
        assert!(result.is_ok());

        // Drop the lock explicitly
        {
            let sender_lock = app_request.resp_tx.read().unwrap();
            assert!(sender_lock.is_none());
        } // Lock is released here

        // Assert that the response has been sent through the channel
        let received_response = receiver.await.unwrap();
        assert_eq!(received_response, response);
    }

    #[tokio::test]
    async fn test_send_response_sender_missing() {
        // Create a mock response
        let response = Ok(AppManagerResponse::None);

        // Create an instance of AppRequest with a None sender
        let app_request = AppRequest {
            method: AppMethod::Ready(String::from("ready")),
            resp_tx: Arc::new(RwLock::new(None)),
        };

        // Call the send_response function
        let result = app_request.send_response(response);

        // Assert that the result is Err with RippleError::SenderMissing
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), RippleError::SenderMissing);
    }

    #[test]
    fn test_extn_payload_provider_for_app_response() {
        let app_response: AppResponse = Ok(AppManagerResponse::State(LifecycleState::Initializing));
        let contract_type: RippleContract = RippleContract::LifecycleManagement;
        test_extn_payload_provider(app_response, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_app_event_request() {
        let app_event_request = AppEventRequest::Emit(AppEvent {
            event_name: String::from("your_event_name"),
            result: serde_json::to_value("your_event_result").unwrap(),
            context: Some(serde_json::to_value("your_event_context").unwrap()),
            app_id: Some(String::from("your_app_id")),
        });

        let contract_type: RippleContract = RippleContract::AppEvents;
        test_extn_payload_provider(app_event_request, contract_type);
    }
}
