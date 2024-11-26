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

use futures::{future::join_all, StreamExt};
use jsonrpsee::{
    core::{
        server::{
            helpers::MethodSink,
            resource_limiting::Resources,
            rpc_module::{MethodKind, Methods},
        },
        TEN_MB_SIZE_BYTES,
    },
    types::{error::ErrorCode, Id, Params},
};
use ripple_sdk::{
    api::{
        firebolt::fb_metrics::Timer,
        gateway::rpc_gateway_api::{ApiMessage, ApiStats, RpcRequest},
    },
    chrono::Utc,
    extn::extn_client_message::ExtnMessage,
    log::{error, info},
    tokio,
    utils::error::RippleError,
};
use std::sync::{Arc, RwLock};

use crate::{
    firebolt::firebolt_gateway::JsonRpcMessage,
    service::telemetry_builder::TelemetryBuilder,
    state::{platform_state::PlatformState, session_state::Session},
    utils::router_utils::{
        add_telemetry_status_code, capture_stage, get_rpc_header, return_api_message_for_transport,
        return_extn_response,
    },
};

pub struct RpcRouter;

#[derive(Debug, Clone)]
pub struct RouterState {
    methods: Arc<RwLock<Methods>>,
    resources: Resources,
}

impl RouterState {
    pub fn new() -> RouterState {
        RouterState {
            methods: Arc::new(RwLock::new(Methods::new())),
            resources: Resources::default(),
        }
    }

    pub fn update_methods(&self, methods: Methods) {
        let mut methods_state = self.methods.write().unwrap();
        let _ = methods_state.merge(methods.initialize_resources(&self.resources).unwrap());
    }

    fn get_methods(&self) -> Methods {
        self.methods.read().unwrap().clone()
    }
}

impl Default for RouterState {
    fn default() -> Self {
        Self::new()
    }
}

async fn resolve_route(
    methods: Methods,
    resources: Resources,
    req: RpcRequest,
) -> Result<ApiMessage, RippleError> {
    info!("Routing {}", req.method);
    let id = Id::Number(req.ctx.call_id);
    let mut request_c = req.clone();
    let (sink_tx, mut sink_rx) = futures_channel::mpsc::unbounded::<String>();
    let sink = MethodSink::new_with_limit(sink_tx, TEN_MB_SIZE_BYTES);
    let mut method_executors = Vec::new();
    let params = Params::new(Some(req.params_json.as_str()));
    match methods.method_with_name(&req.method) {
        None => {
            sink.send_error(id, ErrorCode::MethodNotFound.into());
        }
        Some((name, method)) => match &method.inner() {
            MethodKind::Sync(callback) => match method.claim(name, &resources) {
                Ok(_guard) => {
                    (callback)(id, params, &sink);
                }
                Err(_) => {
                    sink.send_error(id, ErrorCode::MethodNotFound.into());
                }
            },
            MethodKind::Async(callback) => match method.claim(name, &resources) {
                Ok(guard) => {
                    let sink = sink.clone();
                    let id = id.into_owned();
                    let params = params.into_owned();
                    let fut = async move {
                        (callback)(id, params, sink, 1, Some(guard)).await;
                    };
                    method_executors.push(fut);
                }
                Err(e) => {
                    error!("{:?}", e);
                    sink.send_error(id, ErrorCode::MethodNotFound.into());
                }
            },
            _ => {
                error!("Unsupported method call");
            }
        },
    }

    join_all(method_executors).await;
    if let Some(r) = sink_rx.next().await {
        let rpc_header = get_rpc_header(&req);
        let protocol = req.ctx.protocol.clone();
        let request_id = req.ctx.request_id;

        let status_code = if let Ok(r) = serde_json::from_str::<JsonRpcMessage>(&r) {
            if let Some(ec) = r.error {
                ec.code
            } else {
                1
            }
        } else {
            1
        };
        capture_stage(&mut request_c, "routing");

        let mut msg = ApiMessage::new(protocol, r, request_id);
        msg.stats = Some(ApiStats {
            stats_ref: add_telemetry_status_code(&rpc_header, status_code.to_string().as_str()),
            stats: request_c.stats,
        });

        return Ok(msg);
    }
    Err(RippleError::InvalidOutput)
}

impl RpcRouter {
    pub async fn route(
        state: PlatformState,
        mut req: RpcRequest,
        session: Session,
        timer: Option<Timer>,
    ) {
        let methods = state.router_state.get_methods();
        let resources = state.router_state.resources.clone();

        if let Some(overridden_method) = state.get_manifest().has_rpc_override_method(&req.method) {
            req.method = overridden_method;
        }

        tokio::spawn(async move {
            let start = Utc::now().timestamp_millis();
            let resp = resolve_route(methods, resources, req.clone()).await;

            let status = match resp.clone() {
                Ok(msg) => {
                    if msg.is_error() {
                        msg.jsonrpc_msg
                    } else {
                        "0".into()
                    }
                }
                Err(e) => format!("{}", e),
            };

            TelemetryBuilder::stop_and_send_firebolt_metrics_timer(&state, timer, status).await;

            if let Ok(msg) = resp {
                let now = Utc::now().timestamp_millis();
                let success = !msg.is_error();
                TelemetryBuilder::send_fb_tt(&state, req.clone(), now - start, success, &msg);

                return_api_message_for_transport(session, msg, state).await;
            }
        });
    }

    pub async fn route_extn_protocol(
        state: &PlatformState,
        req: RpcRequest,
        extn_msg: ExtnMessage,
    ) {
        let methods = state.router_state.get_methods();
        let resources = state.router_state.resources.clone();
        tokio::spawn(async move {
            if let Ok(msg) = resolve_route(methods, resources, req).await {
                return_extn_response(msg, extn_msg);
            }
        });
    }
}
