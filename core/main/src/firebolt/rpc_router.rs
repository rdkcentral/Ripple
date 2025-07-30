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

use futures::StreamExt;
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
        gateway::rpc_gateway_api::{ApiMessage, RpcRequest},
        observability::log_signal::LogSignal,
    },
    chrono::Utc,
    extn::extn_client_message::ExtnMessage,
    log::{debug, error, info},
    tokio,
    utils::error::RippleError,
};
use std::sync::{Arc, RwLock};

use crate::{
    firebolt::firebolt_gateway::JsonRpcMessage,
    service::telemetry_builder::TelemetryBuilder,
    state::{platform_state::PlatformState, session_state::Session},
    utils::router_utils::{
        add_telemetry_status_code, capture_stage, get_rpc_header, return_extn_response,
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
    platform_state: PlatformState,
    methods: Methods,
    resources: Resources,
    req: RpcRequest,
) -> Result<ApiMessage, RippleError> {
    info!("Routing {}", req.method);
    let id = Id::Number(req.ctx.call_id);
    let request_c = req.clone();
    let sink_size = 1024 * 1024;
    let (sink_tx, mut sink_rx) = futures_channel::mpsc::unbounded::<String>();
    let sink = MethodSink::new_with_limit(sink_tx, TEN_MB_SIZE_BYTES, 512 * 1024);
    let method = request_c.method.clone();

    tokio::spawn(async move {
        let params_json = request_c.params_json.as_ref();
        let params = Params::new(Some(params_json));

        match methods.method_with_name(&method) {
            None => {
                LogSignal::new(
                    "rpc_router".to_string(),
                    "resolve_route".into(),
                    request_c.clone(),
                )
                .with_diagnostic_context_item("error", &format!("Method not found: {}", method))
                .emit_error();
                sink.send_error(id, ErrorCode::MethodNotFound.into());
            }
            Some((name, method)) => match &method.inner() {
                MethodKind::Sync(callback) => match method.claim(name, &resources) {
                    Ok(_guard) => {
                        if let Err(e) =
                            sink.send_raw((callback)(id.clone(), params, 512 * 1024).result)
                        {
                            LogSignal::new(
                                "rpc_router".to_string(),
                                "resolve_route".into(),
                                request_c.clone(),
                            )
                            .with_diagnostic_context_item(
                                "error",
                                &format!("Sync method error: {}", e),
                            )
                            .emit_error();
                            sink.send_error(id, ErrorCode::InvalidRequest.into());
                        }
                    }
                    Err(_) => {
                        sink.send_error(id, ErrorCode::MethodNotFound.into());
                    }
                },
                MethodKind::Async(callback) => match method.claim(name, &resources) {
                    Ok(guard) => {
                        let id = id.into_owned();
                        let params = params.into_owned();
                        if let Err(e) = sink.send_raw(
                            (callback)(id.clone(), params, sink_size, 1024 * 1024, Some(guard))
                                .await
                                .result,
                        ) {
                            LogSignal::new(
                                "rpc_router".to_string(),
                                "resolve_route".into(),
                                request_c.clone(),
                            )
                            .with_diagnostic_context_item(
                                "error",
                                &format!("Async method error: {}", e),
                            )
                            .emit_error();
                            sink.send_error(id, ErrorCode::InvalidRequest.into());
                        }
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
    });

    if let Some(r) = sink_rx.next().await {
        debug!("Received response from method sink {}", r.clone());
        let rpc_header = get_rpc_header(&req);
        let protocol = req.ctx.protocol.clone();
        let request_id = req.clone().ctx.request_id;

        let status_code = if let Ok(r) = serde_json::from_str::<JsonRpcMessage>(&r) {
            if let Some(ec) = r.error {
                ec.code
            } else {
                1
            }
        } else {
            1
        };

        capture_stage(platform_state.clone(), &req, "routing").await;
        platform_state
            .update_api_stats_ref(
                &request_id,
                add_telemetry_status_code(&rpc_header, status_code.to_string().as_str()),
            )
            .await;

        let mut msg = ApiMessage::new(protocol, r, request_id.clone());
        if let Some(api_stats) = platform_state.get_api_stats(&request_id).await {
            msg.stats = Some(api_stats);
        }

        return Ok(msg);
    }
    error!("Invalid output from method sink");
    Err(RippleError::InvalidOutput)
}

impl RpcRouter {
    pub async fn route(state: PlatformState, mut req: RpcRequest, session: Session) {
        let methods = state.router_state.get_methods();
        let resources = state.router_state.resources.clone();

        if let Some(overridden_method) = state.get_manifest().has_rpc_override_method(&req.method) {
            req.method = overridden_method;
        }
        LogSignal::new("rpc_router".to_string(), "routing".into(), req.clone());
        tokio::spawn(async move {
            let start = Utc::now().timestamp_millis();
            let resp = resolve_route(state.clone(), methods, resources, req.clone()).await;
            if let Ok(msg) = resp {
                let now = Utc::now().timestamp_millis();
                let success = !msg.is_error();
                TelemetryBuilder::send_fb_tt(
                    state.clone(),
                    req.clone(),
                    now - start,
                    success,
                    &msg,
                )
                .await;
                let _ = session.send_json_rpc(msg).await;
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

        LogSignal::new(
            "rpc_router".into(),
            "route_extn_protocol".into(),
            req.clone(),
        )
        .emit_debug();
        let platform_state = state.clone();
        tokio::spawn(async move {
            if let Ok(msg) =
                resolve_route(platform_state.clone(), methods, resources, req.clone()).await
            {
                let client = platform_state.clone().get_client().get_extn_client();
                return_extn_response(msg, extn_msg, client);
            } else {
                LogSignal::new("rpc_router".into(), "resolve_route failed".into(), req);
            }
        });
    }
}
