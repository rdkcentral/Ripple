use arrayvec::ArrayVec;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

#[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
use crate::{
    api::handlers::discovery::LaunchRequest,
    apps::app_library::AppLibrary,
    containers::{
        container_message::{ContainerMethod, ContainerRequest, ContainerResponse},
        container_mgr::{ContainerError, ContainerProperties},
    },
    managers::view_manager::Dimensions,
};

use crate::helpers::ripple_helper::IRippleHelper;

use crate::{
    api::{
        handlers::{
            entertainment_data::{
                EntityInfoParameters, EntityInfoResult, ProviderResult, PurchasedContentParameters,
                PurchasedContentResult,
            },
            keyboard::{KeyboardResult, KeyboardSession},
            pin_challenge::{PinChallengeRequest, PinChallengeResponse},
        },
        permissions::user_grants::{Challenge, ChallengeResponse},
        rpc::rpc_gateway::CallContext,
    },
    apps::app_events::AppEvents,
    helpers::channel_util::oneshot_send_and_log,
    managers::capability_manager::FireboltCap,
    platform_state::PlatformState,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use super::app_events::ListenRequest;

#[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
use super::app_mgr::{AppError, AppManagerResponse, AppMethod, AppRequest};

// TODO: This needs definition in the Firebolt spec. Is this the navigation intent action or context?
#[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
pub const NAVIGATION_INTENT_PROVIDER_REQUEST: &'static str = "providerRequest";

const REQUEST_QUEUE_CAPACITY: usize = 3;

#[derive(Serialize, Deserialize, Debug)]
pub enum ProviderError {
    General,
    NotFound,
    NotSupported,
    IoError,
}

#[derive(Clone, Default)]
pub struct ProviderBrokerState {
    provider_methods: Arc<RwLock<HashMap<String, ProviderMethod>>>,
    active_sessions: Arc<RwLock<HashMap<String, ProviderSession>>>,
    request_queue: Arc<RwLock<ArrayVec<Request, REQUEST_QUEUE_CAPACITY>>>,
}

impl std::fmt::Debug for ProviderBrokerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderBrokerState").finish()
    }
}

pub struct ProviderBroker {}

#[derive(Clone)]
struct ProviderMethod {
    event_name: &'static str,
    provider: CallContext,
}

struct ProviderSession {
    caller: ProviderCaller,
    provider: ProviderMethod,
    capability: String,
    focused: bool,
}

#[derive(Debug)]
pub struct Request {
    pub capability: String,
    pub method: String,
    pub caller: CallContext,
    pub request: ProviderRequestPayload,
    pub tx: oneshot::Sender<ProviderResponsePayload>,
    pub app_id: Option<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ProviderRequestPayload {
    KeyboardSession(KeyboardSession),
    PinChallenge(PinChallengeRequest),
    AckChallenge(Challenge),
    EntityInfoRequest(EntityInfoParameters),
    PurchasedContentRequest(PurchasedContentParameters),
    Generic(String),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ProviderResponsePayload {
    KeyboardResult(KeyboardResult),
    ChallengeResponse(ChallengeResponse),
    PinChallengeResponse(PinChallengeResponse),
    EntityInfoResponse(Option<EntityInfoResult>),
    PurchasedContentResponse(PurchasedContentResult),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRequest {
    pub correlation_id: String,
    pub parameters: ProviderRequestPayload,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProviderResponse {
    pub correlation_id: String,
    pub result: ProviderResponsePayload,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProviderRequest<T> {
    pub correlation_id: String,
    pub parameters: T,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalProviderResponse<T> {
    pub correlation_id: String,
    pub result: T,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FocusRequest {
    pub correlation_id: String,
}

impl ProviderResponsePayload {
    pub fn as_keyboard_result(&self) -> Option<KeyboardResult> {
        match self {
            ProviderResponsePayload::KeyboardResult(res) => Some(res.clone()),
            _ => None,
        }
    }
    pub fn as_challenge_response(&self) -> Option<ChallengeResponse> {
        match self {
            ProviderResponsePayload::ChallengeResponse(res) => Some(res.clone()),
            ProviderResponsePayload::PinChallengeResponse(res) => Some(ChallengeResponse {
                granted: res.get_granted(),
            }),
            _ => None,
        }
    }
    pub fn as_pin_challenge_response(&self) -> Option<PinChallengeResponse> {
        match self {
            ProviderResponsePayload::PinChallengeResponse(res) => Some(res.clone()),
            _ => None,
        }
    }
    pub fn as_entity_info_result(&self) -> Option<Option<EntityInfoResult>> {
        match self {
            ProviderResponsePayload::EntityInfoResponse(res) => Some(res.clone()),
            _ => None,
        }
    }
    pub fn as_purchased_content_result(&self) -> Option<PurchasedContentResult> {
        match self {
            ProviderResponsePayload::PurchasedContentResponse(res) => Some(res.clone()),
            _ => None,
        }
    }
}

struct ProviderCaller {
    identity: CallContext,
    tx: oneshot::Sender<ProviderResponsePayload>,
}

impl ProviderBroker {
    pub async fn register_or_unregister_provider(
        pst: &PlatformState,
        capability: String,
        method: String,
        event_name: &'static str,
        provider: CallContext,
        listen_request: ListenRequest,
    ) {
        if listen_request.listen {
            ProviderBroker::register_provider(
                pst,
                capability,
                method,
                event_name,
                provider,
                listen_request,
            )
            .await;
        } else {
            ProviderBroker::unregister_provider(pst, capability, method, provider).await;
        }
    }

    pub async fn unregister_provider(
        pst: &PlatformState,
        capability: String,
        method: String,
        provider: CallContext,
    ) {
        let pst_c = pst.clone();
        {
            let mut provider_methods = pst.provider_broker_state.provider_methods.write().unwrap();
            let cap_method = format!("{}:{}", capability, method);
            if let Some(method) = provider_methods.get(&cap_method) {
                // unregister the capability if it is provided by the session
                // that is making the unregister call
                if method.provider.session_id == provider.session_id {
                    provider_methods.remove(&cap_method);
                }
                ProviderBroker::remove_request(&pst, &provider.app_id.clone(), &capability);
            }
        }
        if let Err(e) = pst_c
            .services
            .update_cam(
                FireboltCap::Full(capability),
                crate::managers::capability_manager::Availability::NotReady,
            )
            .await
        {
            error!("failed to update cam {:?}", e);
        }
    }

    #[instrument(skip(pst))]
    pub async fn register_provider(
        pst: &PlatformState,
        capability: String,
        method: String,
        event_name: &'static str,
        provider: CallContext,
        listen_request: ListenRequest,
    ) {
        let pst_c = pst.clone();
        debug!(
            "register_provider: capability={}, method={}, event_name={}",
            capability, method, event_name
        );
        let cap_method = format!("{}:{}", capability, method);
        let provider_app_id = provider.app_id.clone();
        AppEvents::add_listener(
            &pst.app_events_state,
            event_name.to_string(),
            provider.clone(),
            listen_request,
        );
        {
            let mut provider_methods = pst.provider_broker_state.provider_methods.write().unwrap();
            provider_methods.insert(
                cap_method,
                ProviderMethod {
                    event_name,
                    provider,
                },
            );
        }
        let existing = ProviderBroker::remove_request(&pst, &provider_app_id, &capability);
        if let Some(request) = existing {
            info!("register_provider: Found pending provider request, invoking");
            ProviderBroker::invoke_method(&pst, request).await;
        }

        if let Err(e) = pst_c
            .services
            .update_cam(
                FireboltCap::Full(capability),
                crate::managers::capability_manager::Availability::Ready,
            )
            .await
        {
            error!("error updating cam {:?}", e);
        }
    }

    #[instrument]
    pub fn get_provider_methods(pst: &PlatformState) -> ProviderResult {
        let provider_methods = pst.provider_broker_state.provider_methods.read().unwrap();
        let mut result: HashMap<String, Vec<String>> = HashMap::new();
        let caps_keys = provider_methods.keys();
        let all_caps = caps_keys.cloned().collect::<Vec<String>>();
        for cap in all_caps {
            if let Some(provider) = provider_methods.get(&cap) {
                if let Some(list) = result.get_mut(&provider.provider.app_id) {
                    list.push(String::from(provider.event_name));
                } else {
                    result.insert(
                        provider.provider.app_id.clone(),
                        vec![String::from(provider.event_name)],
                    );
                }
            }
        }
        ProviderResult::new(result)
    }

    pub async fn invoke_method(pst: &PlatformState, request: Request) {
        let cap_method = format!("{}:{}", request.capability, request.method);
        debug!("invoking provider for {}", cap_method);

        let provider_opt = {
            let provider_methods = pst.provider_broker_state.provider_methods.read().unwrap();
            provider_methods.get(&cap_method).cloned()
        };
        if let Some(provider) = provider_opt {
            let event_name = provider.event_name.clone();
            let req_params = request.request.clone();
            let app_id_opt = request.app_id.clone();
            let c_id = ProviderBroker::start_provider_session(&pst, request, provider);
            if let Some(app_id) = app_id_opt {
                debug!("Sending request to specific app {}", app_id);
                AppEvents::emit_to_app(
                    &pst,
                    app_id,
                    event_name,
                    &serde_json::to_value(ProviderRequest {
                        correlation_id: c_id,
                        parameters: req_params,
                    })
                    .unwrap(),
                )
                .await;
            } else {
                debug!("Broadcasting request to all the apps!!");
                AppEvents::emit(
                    pst,
                    event_name,
                    &serde_json::to_value(ProviderRequest {
                        correlation_id: c_id,
                        parameters: req_params,
                    })
                    .unwrap(),
                )
                .await;
            }
        } else {
            #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
            let _cap = request.capability.clone();
            ProviderBroker::queue_provider_request(pst, request);

            #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
            ProviderBroker::launch_provider(pst, _cap).await.ok();
        }
    }

    fn start_provider_session(
        pst: &PlatformState,
        request: Request,
        provider: ProviderMethod,
    ) -> String {
        let c_id = Uuid::new_v4().to_string();
        let mut active_sessions = pst.provider_broker_state.active_sessions.write().unwrap();
        active_sessions.insert(
            c_id.clone(),
            ProviderSession {
                caller: ProviderCaller {
                    identity: request.caller,
                    tx: request.tx,
                },
                provider: provider.clone(),
                capability: request.capability,
                focused: false,
            },
        );
        c_id
    }

    fn queue_provider_request(pst: &PlatformState, request: Request) {
        // Remove any duplicate requests.
        ProviderBroker::remove_request(pst, &request.caller.app_id, &request.capability);

        let mut request_queue = pst.provider_broker_state.request_queue.write().unwrap();
        if request_queue.is_full() {
            warn!("invoke_method: Request queue full, removing oldest request");
            request_queue.remove(0);
        }
        request_queue.push(request);
    }

    #[instrument]
    pub async fn provider_response(pst: &PlatformState, resp: ProviderResponse) {
        let cap_with_focused_ui = {
            let mut cap_with_focused_ui = None;
            let mut active_sessions = pst.provider_broker_state.active_sessions.write().unwrap();
            match active_sessions.remove(&resp.correlation_id) {
                Some(session) => {
                    if session.focused {
                        cap_with_focused_ui = Some(session.capability.clone())
                    }
                    oneshot_send_and_log(session.caller.tx, resp.result, "ProviderResponse");
                }
                None => {
                    error!("Ignored provider response because there was no active session waiting")
                }
            }
            cap_with_focused_ui
        };

        if let Some(_cap) = cap_with_focused_ui {
            #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
            ProviderBroker::remove_ui(pst, _cap).await;
        }
    }

    fn cleanup_caps_for_unregister(pst: &PlatformState, session_id: String) -> Vec<String> {
        let mut active_sessions = pst.provider_broker_state.active_sessions.write().unwrap();
        let cid_keys = active_sessions.keys();
        let all_cids = cid_keys.cloned().collect::<Vec<String>>();
        let mut clear_cids = Vec::<String>::new();
        // find all the sessions where either the caller or the provider are being unregistered and clear that session
        // the oneshot for the caller should then get descoped and called with an error
        for cid in all_cids {
            if let Some(session) = active_sessions.get(&cid) {
                if session.caller.identity.session_id == session_id
                    || session.provider.provider.session_id == session_id
                {
                    clear_cids.push(cid);
                }
            }
        }

        for cid in clear_cids {
            active_sessions.remove(&cid);
        }
        let mut provider_methods = pst.provider_broker_state.provider_methods.write().unwrap();
        // find all providers for the session being unregistered
        // remove the provided capability
        let mut clear_caps = Vec::new();
        let caps_keys = provider_methods.keys();
        let all_caps = caps_keys.cloned().collect::<Vec<String>>();
        for cap in all_caps {
            if let Some(provider) = provider_methods.get(&cap) {
                if provider.provider.session_id == session_id {
                    clear_caps.push(cap);
                }
            }
        }
        for cap in clear_caps.clone() {
            provider_methods.remove(&cap);
        }
        clear_caps
    }

    #[instrument]
    pub async fn unregister_session(pst: &PlatformState, session_id: String) {
        let cleaned_caps = Self::cleanup_caps_for_unregister(&pst.clone(), session_id);
        let mut caps = HashSet::<FireboltCap>::new();
        for cap in cleaned_caps {
            if let Some(f) = FireboltCap::parse(cap.clone()) {
                caps.insert(f);
            }
        }

        for f in caps {
            let cap_str = f.as_str();
            if let Err(e) = pst
                .clone()
                .services
                .update_cam(
                    f,
                    crate::managers::capability_manager::Availability::NotReady,
                )
                .await
            {
                error!("{} provider not unregistered {:?}", cap_str, e);
            }
        }
    }

    #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
    #[instrument]
    async fn display_ui(pst: &PlatformState, ctx: CallContext, capability: String) {
        debug!("display_ui: capability={}", capability);
        let resp = ProviderBroker::app_request(pst, AppMethod::GetViewId(ctx.app_id)).await;
        if let Err(e) = resp {
            error!("display_ui: Error: e={:?}", e);
            return;
        }

        let view_id;
        match resp.unwrap() {
            AppManagerResponse::ViewId(uuid) => view_id = uuid,
            _ => {
                error!("display_ui: Unexpected type");
                return;
            }
        }

        let props = ContainerProperties {
            name: capability,
            view_id,
            requires_focus: true,
            dimensions: Dimensions {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
            },
        };

        ProviderBroker::container_request(pst, ContainerMethod::Add(props)).await;
    }

    #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
    #[instrument]
    async fn remove_ui(pst: &PlatformState, capability: String) {
        ProviderBroker::container_request(pst, ContainerMethod::Remove(capability)).await;
    }

    #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
    #[instrument]
    async fn container_request(pst: &PlatformState, method: ContainerMethod) -> ContainerResponse {
        let (container_resp_tx, container_resp_rx) = oneshot::channel::<ContainerResponse>();
        let container_req = ContainerRequest {
            method,
            resp_tx: container_resp_tx,
        };

        if let Err(_e) = pst.services.send_container_request(container_req).await {
            error!("container_request: Send error");
            return ContainerResponse {
                status: Err(ContainerError::IoError),
            };
        }

        let resp = container_resp_rx.await;
        if let Err(_e) = resp {
            error!("container_request: Receive error");
            return ContainerResponse {
                status: Err(ContainerError::IoError),
            };
        }

        resp.unwrap()
    }

    #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
    #[instrument]
    async fn app_request(
        pst: &PlatformState,
        method: AppMethod,
    ) -> Result<AppManagerResponse, AppError> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();
        let app_req = AppRequest {
            method,
            resp_tx: Some(app_resp_tx),
        };

        if let Err(_e) = pst.services.send_app_request(app_req).await {
            error!("app_request: Send error");
            return Err(AppError::IoError);
        }

        let resp = app_resp_rx.await;
        if let Err(_e) = resp {
            error!("contaiapp_requestner_request: Receive error");
            return Err(AppError::IoError);
        }

        resp.unwrap()
    }

    #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
    #[instrument]
    async fn launch_provider(pst: &PlatformState, capability: String) -> Result<(), ProviderError> {
        debug!("launch_provider: capability={}", capability);
        let resp = AppLibrary::get_provider(&pst.app_library_state, capability.clone());
        if let None = resp {
            warn!(%capability, "launch_provider: Provider not found");
            return Err(ProviderError::NotFound);
        }

        match ProviderBroker::app_request(
            pst,
            AppMethod::Launch(LaunchRequest::new(
                resp.unwrap(),
                NAVIGATION_INTENT_PROVIDER_REQUEST.to_string(),
                None,
                NAVIGATION_INTENT_PROVIDER_REQUEST.to_string(),
            )),
        )
        .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(ProviderError::General),
        }
    }

    #[instrument]
    fn remove_request(
        pst: &PlatformState,
        provider_id: &String,
        capability: &String,
    ) -> Option<Request> {
        let mut request_queue = pst.provider_broker_state.request_queue.write().unwrap();
        let mut iter = request_queue.iter();
        let cap = iter.position(|request| request.capability.eq(capability));
        if let Some(index) = cap {
            let request = request_queue.remove(index);
            return Some(request);
        }
        None
    }

    pub async fn focus(
        pst: &PlatformState,
        _ctx: CallContext,
        _capability: String,
        request: FocusRequest,
    ) {
        let needs_ui = {
            let mut active_sessions = pst.provider_broker_state.active_sessions.write().unwrap();
            if let Some(session) = active_sessions.get_mut(&request.correlation_id) {
                session.focused = true;
                true
            } else {
                warn!("Focus: No active session for request");
                false
            }
        };

        if needs_ui {
            #[cfg(any(feature = "only_launcher", feature = "gateway_with_launcher"))]
            ProviderBroker::display_ui(pst, _ctx, _capability).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        api::rpc::firebolt_gateway::tests::TestGateway, apps::app_events::ListenRequest,
        platform_state::PlatformState,
    };

    use super::ProviderBroker;

    #[tokio::test]
    async fn test_register_and_unregister_provider() {
        let pst = PlatformState::default();
        ProviderBroker::register_or_unregister_provider(
            &pst,
            "xrn:firebolt:cap".into(),
            "my_method".into(),
            "my_event",
            TestGateway::consumer_call(),
            ListenRequest { listen: true },
        )
        .await;
        let res = ProviderBroker::get_provider_methods(&pst);
        assert!(res.entries.get("app_consumer").is_some());
        assert!(res
            .entries
            .get("app_consumer")
            .unwrap()
            .contains(&"my_event".to_string()));

        ProviderBroker::register_or_unregister_provider(
            &pst,
            "xrn:firebolt:cap".into(),
            "my_method".into(),
            "my_event",
            TestGateway::consumer_call(),
            ListenRequest { listen: false },
        )
        .await;
        let res = ProviderBroker::get_provider_methods(&pst);
        assert!(res.entries.get("app_consumer").is_none());
    }
}
