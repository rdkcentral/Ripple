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

use crate::{
    api::{
        gateway::rpc_gateway_api::{ApiMessage, RpcRequest},
        manifest::extn_manifest::ExtnManifest,
    },
    log::{error, info},
    utils::error::RippleError,
    utils::router_utils::get_rpc_header,
};
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
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

pub struct RpcRouter;

#[derive(Debug, Clone)]
pub struct RouterState {
    methods: Arc<RwLock<Methods>>,
    resources: Resources,
    manifest: ExtnManifest,
}

impl RouterState {
    pub fn new() -> RouterState {
        RouterState {
            methods: Arc::new(RwLock::new(Methods::new())),
            resources: Resources::default(),
            manifest: ExtnManifest::default(),
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

#[derive(Serialize, Deserialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

impl RpcRouter {
    pub async fn resolve_route(
        req: RpcRequest,
        router_state: &RouterState,
    ) -> Result<ApiMessage, RippleError> {
        println!("**** rpc_router: resolve_route: routing req: {:?}", req);
        println!("**** rpc_router: resolve_route: routing {}", req.method);
        info!("Routing {}", req.method);
        let id = Id::Number(req.ctx.call_id);
        let methods = router_state.get_methods();
        let resources = router_state.resources.clone();
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
                        println!("**** rpc_router: resolve_route: MethodKind::Sync");
                        (callback)(id, params, &sink);
                    }
                    Err(_) => {
                        sink.send_error(id, ErrorCode::MethodNotFound.into());
                    }
                },
                MethodKind::Async(callback) => match method.claim(name, &resources) {
                    Ok(guard) => {
                        println!("**** rpc_router: resolve_route: MethodKind::ASync");
                        let sink = sink.clone();
                        let id = id.into_owned();
                        let params = params.into_owned();
                        let fut = async move {
                            println!("**** rpc_router: resolve_route: MethodKind::ASync fut");
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

            let mut msg = ApiMessage::new(protocol, r, request_id.clone());
            println!(
                "**** rpc_router: resolve_route: returning msg: {:?}",
                msg.clone()
            );

            return Ok(msg);
        }
        Err(RippleError::InvalidOutput)
    }
}
