use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::{
    extn::{
        extn_capability::ExtnCapability,
        extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    },
    utils::{channel_utils::oneshot_send_and_log, error::RippleError},
};

use super::firebolt::{
    fb_discovery::{LaunchRequest, NavigationIntent},
    fb_lifecycle::LifecycleState,
    fb_parameters::SecondScreenEvent,
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
}

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

    fn cap() -> ExtnCapability {
        ExtnCapability::get_main_target("lifecycle-management".into())
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

impl From<AppError> for jsonrpsee::core::error::Error {
    fn from(err: AppError) -> Self {
        jsonrpsee::core::error::Error::Custom(format!("Internal failure: {:?}", err))
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
