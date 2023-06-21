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
        fb_parameters::SecondScreenEvent,
    },
    gateway::rpc_gateway_api::CallContext,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSession {
    pub app: AppBasicInfo,
    pub runtime: Option<AppRuntime>,
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppBasicInfo {
    pub id: String,
    pub catalog: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum AppRuntimeTransport {
    Bridge,
    Websocket,
}

fn runtime_transport_default() -> AppRuntimeTransport {
    AppRuntimeTransport::Websocket
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppRuntime {
    pub id: Option<String>,
    #[serde(default = "runtime_transport_default")]
    pub transport: AppRuntimeTransport,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppLaunchInfo {
    pub intent: NavigationIntent,
    #[serde(rename = "secondScreen")]
    pub second_screen: Option<SecondScreenEvent>,
    #[serde(default = "bool::default")]
    pub inactive: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EffectiveTransport {
    Bridge(String),
    Websocket,
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
        match payload {
            ExtnPayload::Response(resp) => match resp {
                ExtnResponse::Value(v) => {
                    if let Ok(v) = serde_json::from_value(v) {
                        return Some(Ok(v));
                    }
                }
                ExtnResponse::Error(_) => return Some(Err(AppError::General)),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
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
}

impl From<AppError> for jsonrpsee_core::Error {
    fn from(err: AppError) -> Self {
        jsonrpsee_core::Error::Custom(format!("Internal failure: {:?}", err))
    }
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum CloseReason {
    RemoteButton,
    UserExit,
    Error,
    AppNotReady,
    ResourceContention,
}

impl CloseReason {
    pub fn as_string(&self) -> &'static str {
        match self {
            CloseReason::RemoteButton => "remoteButton",
            CloseReason::UserExit => "userExit",
            CloseReason::Error => "error",
            CloseReason::AppNotReady => "appNotReady",
            CloseReason::ResourceContention => "resourceContention",
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct StateChange {
    pub previous: LifecycleState,
    pub state: LifecycleState,
}
pub type ViewId = Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Dimensions {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AppEvent {
    pub event_name: String,
    pub result: Value,
    pub context: Option<Value>,
    pub app_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
        match payload {
            ExtnPayload::Event(resp) => match resp {
                ExtnEvent::Value(v) => {
                    if let Ok(v) = serde_json::from_value(v) {
                        return Some(v);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::AppEvents
    }
}
