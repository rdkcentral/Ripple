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
    api::{gateway::rpc_gateway_api::RpcRequest, observability::log_signal::LogSignal},
    log::{error, trace},
    utils::error::RippleError,
};
use futures::StreamExt;
use jsonrpsee::{
    core::server::{
        helpers::MethodSink,
        resource_limiting::Resources,
        rpc_module::{MethodCallback, MethodKind, Methods},
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
    pub fn get_method_entry(&self, method_name: &str) -> Option<(String, MethodCallback)> {
        // Acquire a read lock without cloning the entire Methods registry
        let methods_guard = self.methods.read().ok()?;
        methods_guard
            .method_with_name(method_name)
            .map(|(name, method)| (name.to_owned(), method.clone()))
    }

    pub fn get_resources(&self) -> Resources {
        self.resources.clone()
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
    ) -> Result<String, RippleError> {
        trace!("SDK: Resolving route for {:?}", req);
        let id = Id::Number(req.ctx.call_id);
        let request_c = req.clone();
        let method_name = request_c.method.clone();
        let method_entry = router_state.get_method_entry(method_name.as_str());
        let resources = router_state.resources.clone();
        let (sink_tx, mut sink_rx) = futures_channel::mpsc::unbounded::<String>();
        let sink = MethodSink::new_with_limit(sink_tx, 1024 * 1024, 100 * 1024);
        let sink_size = 1024 * 1024;
        tokio::spawn(async move {
            let params_json = request_c.params_json.as_ref();
            let params = Params::new(Some(params_json));

            match method_entry {
                None => {
                    LogSignal::new(
                        "rpc_router".to_string(),
                        "resolve_route".into(),
                        request_c.clone(),
                    )
                    .with_diagnostic_context_item(
                        "error",
                        &format!("Method not found: {}", method_name),
                    )
                    .emit_error();
                    sink.send_error(id, ErrorCode::MethodNotFound.into());
                }
                Some((name, method)) => match &method.inner() {
                    MethodKind::Sync(callback) => match method.claim(&name, &resources) {
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
                    MethodKind::Async(callback) => match method.claim(&name, &resources) {
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
            return Ok(r);
        }
        Err(RippleError::InvalidOutput)
    }
}
