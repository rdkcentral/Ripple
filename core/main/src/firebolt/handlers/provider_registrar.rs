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

use std::{
    any::{Any, TypeId},
    backtrace::Backtrace,
    collections::HashMap,
    sync::Arc,
};

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::apps::provider_broker::ProviderBroker,
    state::{self, openrpc_state::ProviderSet, platform_state::PlatformState},
};
use futures_channel::mpsc::{self, Sender};
use jsonrpsee::{
    core::{server::rpc_module::Methods, Error, RpcResult},
    types::Params,
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            fb_openrpc::FireboltOpenRpcMethod,
            fb_pin::PinChallengeResponse,
            provider::{
                ChallengeError, ChallengeResponse, ExternalProviderResponse, FocusRequest,
                ProviderAttributes, ProviderResponse, ProviderResponsePayload,
                ACKNOWLEDGE_CHALLENGE_ATTRIBS, ACK_CHALLENGE_CAPABILITY, ACK_CHALLENGE_EVENT,
            },
        },
        gateway::rpc_gateway_api::CallContext,
    },
    log::debug,
};
use serde_json::json;

#[derive(Debug)]
pub struct Provider {
    pub platform_state: PlatformState,
    capability: String,
    event: &'static str,
}

impl Provider {
    pub fn new(platform_state: PlatformState, capability: String, event: &'static str) -> Provider {
        Provider {
            platform_state,
            capability,
            event,
        }
    }

    async fn on_request(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        debug!("on_request: request={:?}", request);
        ProviderBroker::register_or_unregister_provider(
            &self.platform_state,
            self.capability.clone(),
            ProviderBroker::get_method(&self.capability).unwrap_or_default(),
            self.event,
            ctx,
            request,
        )
        .await;

        Ok(ListenerResponse {
            listening: listen,
            event: self.event.into(),
        })
    }

    async fn response(&self, _ctx: CallContext, resp: ProviderResponse) -> RpcResult<Option<()>> {
        ProviderBroker::provider_response(&self.platform_state, resp).await;
        return Ok(None);
    }

    async fn error(&self, _ctx: CallContext, resp: ProviderResponse) -> RpcResult<Option<()>> {
        ProviderBroker::provider_response(&self.platform_state, resp).await;
        return Ok(None);
    }

    async fn focus(&self, ctx: CallContext, request: FocusRequest) -> RpcResult<Option<()>> {
        ProviderBroker::focus(&self.platform_state, ctx, self.capability.clone(), request).await;
        Ok(None)
    }
}

pub struct ProviderRegistrar;

impl ProviderRegistrar {
    pub fn register(platform_state: &PlatformState, mut methods: &mut Methods) {
        let provider_map = platform_state.open_rpc_state.get_provider_map();
        for provides in provider_map.clone().keys() {
            println!(
                "*** _DEBUG: ProviderRegistrar::register: provides={}",
                provides
            );
            if let Some(provider_set) = provider_map.get(provides) {
                if let Some(attributes) = provider_set.attributes.clone() {
                    let mut rpc_module = RpcModule::new(platform_state.clone());

                    // Register request function
                    if let Some(method) = provider_set.request.clone() {
                        let request_method =
                            FireboltOpenRpcMethod::name_with_lowercase_module(&method.name).leak();

                        println!("*** _DEBUG: request_method={}", request_method);

                        rpc_module
                            .register_async_method(
                                request_method,
                                move |params, platform_state| async move {
                                    let mut params_sequence = params.sequence();
                                    let call_context: CallContext = params_sequence.next().unwrap();
                                    let request: ListenRequest = params_sequence.next().unwrap();
                                    let listening = request.listen;

                                    ProviderBroker::register_or_unregister_provider(
                                        &platform_state,
                                        attributes.capability.into(),
                                        attributes.method.into(),
                                        attributes.event,
                                        call_context,
                                        request,
                                    )
                                    .await;

                                    Ok(ListenerResponse {
                                        listening,
                                        event: attributes.event.into(),
                                    })
                                },
                            )
                            .unwrap();
                    }

                    // Register focus function
                    if let Some(method) = provider_set.focus.clone() {
                        let focus_method =
                            FireboltOpenRpcMethod::name_with_lowercase_module(&method.name).leak();

                        println!("*** _DEBUG: focus_method={}", focus_method);

                        rpc_module
                            .register_async_method(
                                focus_method,
                                move |params, platform_state| async move {
                                    let mut params_sequence = params.sequence();
                                    let call_context: CallContext = params_sequence.next().unwrap();
                                    let request: FocusRequest = params_sequence.next().unwrap();

                                    ProviderBroker::focus(
                                        &platform_state,
                                        call_context,
                                        attributes.capability.into(),
                                        request,
                                    )
                                    .await;
                                    Ok(None)
                                },
                            )
                            .unwrap();
                    }

                    // Register response function
                    if let Some(method) = provider_set.response.clone() {
                        let response_method =
                            FireboltOpenRpcMethod::name_with_lowercase_module(&method.name).leak();

                        println!("*** _DEBUG: response_method={}", response_method);

                        rpc_module
                            .register_async_method(
                                response_method,
                                move |params, platform_state| async move {
                                    let mut params_sequence = params.sequence();
                                    let call_context: CallContext = params_sequence.next().unwrap();

                                    // <pca> YAH: Make this generic using ProviderAttributes </pca>
                                    let resp: ExternalProviderResponse<ChallengeResponse> =
                                        params_sequence.next().unwrap();

                                    ProviderBroker::provider_response(
                                        &platform_state,
                                        ProviderResponse {
                                            correlation_id: resp.correlation_id,
                                            result: ProviderResponsePayload::ChallengeResponse(
                                                resp.result,
                                            ),
                                        },
                                    )
                                    .await;
                                    Ok(None)
                                },
                            )
                            .unwrap();
                    }

                    // Register error function
                    if let Some(method) = provider_set.error.clone() {
                        let error_method = method.name.clone().leak();
                        rpc_module
                            .register_async_method(error_method, |_params, _ctx| async {
                                println!("*** _DEBUG: error: entry");
                                Ok(())
                            })
                            .unwrap();
                    }

                    methods.merge(rpc_module.clone()).ok();
                }
            }
        }
    }
}
