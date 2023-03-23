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

use arrayvec::ArrayVec;
use ripple_sdk::{
    api::{firebolt::fb_general::ListenRequest, gateway::rpc_gateway_api::CallContext},
    log::{debug, error, info, warn},
    serde_json,
    tokio::sync::oneshot,
    utils::channel_utils::oneshot_send_and_log,
    uuid::Uuid,
};
use serde::{Deserialize, Serialize};

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    service::{apps::app_events::AppEvents, user_grants::ChallengeResponse},
    state::platform_state::PlatformState,
};

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
    _capability: String,
    focused: bool,
}

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
    Generic(String),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ProviderResponsePayload {
    ChallengeResponse(ChallengeResponse),
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

struct ProviderCaller {
    identity: CallContext,
    tx: oneshot::Sender<ProviderResponsePayload>,
}

#[derive(Debug, Serialize, Default, Clone)]
pub struct ProviderResult {
    pub entries: HashMap<String, Vec<String>>,
}

// Adding impl with new function since object is created in provider broker
impl ProviderResult {
    pub fn new(entries: HashMap<String, Vec<String>>) -> Self {
        ProviderResult { entries }
    }
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

        // TODO Add permissions
    }

    pub async fn register_provider(
        pst: &PlatformState,
        capability: String,
        method: String,
        event_name: &'static str,
        provider: CallContext,
        listen_request: ListenRequest,
    ) {
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

        // TODO add Firebolt capabilities
    }

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
            ProviderBroker::queue_provider_request(pst, request);
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
                _capability: request.capability,
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

    pub async fn provider_response(pst: &PlatformState, resp: ProviderResponse) {
        let mut active_sessions = pst.provider_broker_state.active_sessions.write().unwrap();
        match active_sessions.remove(&resp.correlation_id) {
            Some(session) => {
                oneshot_send_and_log(session.caller.tx, resp.result, "ProviderResponse");
            }
            None => {
                error!("Ignored provider response because there was no active session waiting")
            }
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

    pub async fn unregister_session(pst: &PlatformState, session_id: String) {
        let _ = Self::cleanup_caps_for_unregister(&pst.clone(), session_id);
        // TODO: Add permissions
    }

    fn remove_request(
        pst: &PlatformState,
        provider_id: &String,
        capability: &String,
    ) -> Option<Request> {
        info!("Remove request {}", provider_id);
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
        let mut active_sessions = pst.provider_broker_state.active_sessions.write().unwrap();
        if let Some(session) = active_sessions.get_mut(&request.correlation_id) {
            session.focused = true;
        } else {
            warn!("Focus: No active session for request");
        }
    }
}
