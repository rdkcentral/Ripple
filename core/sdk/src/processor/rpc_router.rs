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
    api::gateway::rpc_gateway_api::{ApiMessage, RpcRequest},
    log::{error, info},
    utils::error::RippleError,
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
use std::sync::{Arc, RwLock};

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
        if let Err(e) = methods_state.merge(methods.initialize_resources(&self.resources).unwrap())
        {
            error!("Failed to merge methods: {:?}", e);
        }
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

impl RpcRouter {
    pub async fn resolve_route(
        req: RpcRequest,
        router_state: &RouterState,
    ) -> Result<ApiMessage, RippleError> {
        info!("SDK: Routing {}", req.method);
        let id = Id::Number(req.ctx.call_id);
        let methods = router_state.get_methods();
        let resources = router_state.resources.clone();
        let (sink_tx, mut sink_rx) = futures_channel::mpsc::unbounded::<String>();
        let sink = MethodSink::new_with_limit(sink_tx, TEN_MB_SIZE_BYTES);
        let mut method_executors = Vec::new();
        let params = Params::new(Some(req.params_json.as_str()));

        if let Some((name, method)) = methods.method_with_name(&req.method) {
            match &method.inner() {
                MethodKind::Sync(callback) => {
                    if let Ok(_guard) = method.claim(name, &resources) {
                        callback(id, params, &sink);
                    } else {
                        sink.send_error(id, ErrorCode::MethodNotFound.into());
                    }
                }
                MethodKind::Async(callback) => {
                    if let Ok(guard) = method.claim(name, &resources) {
                        let sink = sink.clone();
                        let id = id.into_owned();
                        let params = params.into_owned();
                        let fut = async move {
                            callback(id, params, sink, 1, Some(guard)).await;
                        };
                        method_executors.push(fut);
                    } else {
                        sink.send_error(id, ErrorCode::MethodNotFound.into());
                    }
                }
                _ => {
                    error!("SDK: Unsupported method call");
                }
            }
        } else {
            sink.send_error(id, ErrorCode::MethodNotFound.into());
        }

        join_all(method_executors).await;

        if let Some(r) = sink_rx.next().await {
            let protocol = req.ctx.protocol.clone();
            let request_id = req.ctx.request_id;

            let msg = ApiMessage::new(
                protocol,
                serde_json::to_string(&r).unwrap(),
                request_id.clone(),
            );

            return Ok(msg);
        }
        Err(RippleError::InvalidOutput)
    }
}
