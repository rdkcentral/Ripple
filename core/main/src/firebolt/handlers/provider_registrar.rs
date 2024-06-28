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
    firebolt::rpc::register_aliases, service::apps::provider_broker::ProviderBroker,
    state::platform_state::PlatformState,
};
use jsonrpsee::{
    core::{server::rpc_module::Methods, RpcResult},
    types::{error::CallError, ParamsSequence},
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            fb_openrpc::FireboltOpenRpcMethod,
            fb_pin::PinChallengeResponse,
            provider::{
                ChallengeResponse, ExternalProviderResponse, FocusRequest, ProviderResponse,
                ProviderResponsePayload, ProviderResponsePayloadType,
            },
        },
        gateway::rpc_gateway_api::CallContext,
    },
    log::error,
};

#[derive(Clone)]
struct RpcModuleContext {
    platform_state: PlatformState,
}

impl RpcModuleContext {
    fn new(platform_state: PlatformState) -> Self {
        RpcModuleContext { platform_state }
    }
}

pub struct ProviderRegistrar;

impl ProviderRegistrar {
    fn get_provider_response(
        payload_type: ProviderResponsePayloadType,
        mut params_sequence: ParamsSequence,
    ) -> Option<ProviderResponse> {
        match payload_type {
            ProviderResponsePayloadType::ChallengeResponse => {
                let external_provider_response: Result<
                    ExternalProviderResponse<ChallengeResponse>,
                    CallError,
                > = params_sequence.next();

                if let Ok(r) = external_provider_response {
                    return Some(ProviderResponse {
                        correlation_id: r.correlation_id,
                        result: ProviderResponsePayload::ChallengeResponse(r.result),
                    });
                }
            }
            ProviderResponsePayloadType::PinChallengeResponse => {
                let external_provider_response: Result<
                    ExternalProviderResponse<PinChallengeResponse>,
                    CallError,
                > = params_sequence.next();

                if let Ok(r) = external_provider_response {
                    return Some(ProviderResponse {
                        correlation_id: r.correlation_id,
                        result: ProviderResponsePayload::PinChallengeResponse(r.result),
                    });
                }
            }
            _ => error!("get_provider_response: Unsupported payload type"),
        }

        None
    }

    pub fn register(platform_state: &PlatformState, methods: &mut Methods) {
        let provider_map = platform_state.open_rpc_state.get_provider_map();
        for provides in provider_map.clone().keys() {
            println!(
                "*** _DEBUG: ProviderRegistrar::register: provides={}",
                provides
            );
            if let Some(provider_set) = provider_map.get(provides) {
                if let Some(attributes) = provider_set.attributes.clone() {
                    println!(
                        "*** _DEBUG: ProviderRegistrar::register: attributes={:?}",
                        attributes
                    );
                    let rpc_module_context = RpcModuleContext::new(platform_state.clone());
                    let mut rpc_module = RpcModule::new(rpc_module_context.clone());

                    // Register request function
                    if let Some(method) = provider_set.request.clone() {
                        let request_method =
                            FireboltOpenRpcMethod::name_with_lowercase_module(&method.name).leak();

                        println!(
                            "*** _DEBUG: ProviderRegistrar::register: request_method={}",
                            request_method
                        );

                        rpc_module
                            .register_async_method(
                                request_method,
                                move |params, context| async move {
                                    println!("*** _DEBUG: async callback: params={:?}", params);

                                    let mut params_sequence = params.sequence();
                                    let call_context: CallContext = params_sequence.next().unwrap();
                                    let request: ListenRequest = params_sequence.next().unwrap();
                                    let listening = request.listen;

                                    ProviderBroker::register_or_unregister_provider(
                                        &context.platform_state,
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
                                move |params, context| async move {
                                    let mut params_sequence = params.sequence();
                                    let call_context: CallContext = params_sequence.next().unwrap();
                                    let request: FocusRequest = params_sequence.next().unwrap();

                                    ProviderBroker::focus(
                                        &context.platform_state,
                                        call_context,
                                        attributes.capability.into(),
                                        request,
                                    )
                                    .await;
                                    Ok(None) as RpcResult<Option<()>>
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
                                move |params, context| async move {
                                    let params_sequence = params.sequence();

                                    if let Some(provider_response) =
                                        ProviderRegistrar::get_provider_response(
                                            attributes.response_payload_type.clone(),
                                            params_sequence,
                                        )
                                    {
                                        ProviderBroker::provider_response(
                                            &context.platform_state,
                                            provider_response,
                                        )
                                        .await;
                                    }

                                    Ok(None) as RpcResult<Option<()>>
                                },
                            )
                            .unwrap();
                    }

                    // Register error function
                    if let Some(method) = provider_set.error.clone() {
                        let error_method = method.name.clone().leak();

                        println!("*** _DEBUG: error_method={}", error_method);

                        rpc_module
                            .register_async_method(
                                error_method,
                                move |params, context| async move {
                                    let params_sequence = params.sequence();

                                    if let Some(provider_response) =
                                        ProviderRegistrar::get_provider_response(
                                            attributes.error_payload_type.clone(),
                                            params_sequence,
                                        )
                                    {
                                        ProviderBroker::provider_response(
                                            &context.platform_state,
                                            provider_response,
                                        )
                                        .await;
                                    }

                                    Ok(None) as RpcResult<Option<()>>
                                },
                            )
                            .unwrap();
                    }

                    //methods.merge(rpc_module.clone()).ok();
                    methods
                        .merge(register_aliases(&platform_state, rpc_module))
                        .ok();
                }
            }
        }
    }
}
