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

use std::time::Duration;

use crate::{
    firebolt::rpc::register_aliases,
    service::apps::{
        app_events::AppEvents,
        provider_broker::{ProviderBroker, ProviderBrokerRequest},
    },
    state::{openrpc_state::ProviderSet, platform_state::PlatformState},
};
use jsonrpsee::{
    core::{server::rpc_module::Methods, Error, RpcResult},
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
                ChallengeResponse, ExternalProviderError, ExternalProviderResponse, FocusRequest,
                ProviderAttributes, ProviderRequestPayload, ProviderResponse,
                ProviderResponsePayload, ProviderResponsePayloadType,
            },
        },
        gateway::rpc_gateway_api::{CallContext, CallerSession},
    },
    log::{error, info},
    tokio::{sync::oneshot, time::timeout},
};
use serde_json::Value;

// <pca>
// TODO: Add to config
const DEFAULT_PROVIDER_RESPONSE_TIMEOUT_MS: u64 = 5000;
// </pca>

#[derive(Clone)]
struct RpcModuleContext {
    platform_state: PlatformState,
    // <pca>
    method: String,
    provider_set: ProviderSet,
    // <pca>
}

impl RpcModuleContext {
    // <pca>
    // fn new(platform_state: PlatformState) -> Self {
    //     RpcModuleContext { platform_state }
    // }
    fn new(platform_state: PlatformState, method: String, provider_set: ProviderSet) -> Self {
        RpcModuleContext {
            method,
            platform_state,
            provider_set,
        }
    }
    // </pca>
}

pub struct ProviderRegistrar;

impl ProviderRegistrar {
    fn get_provider_response(
        payload_type: ProviderResponsePayloadType,
        mut params_sequence: ParamsSequence,
    ) -> Option<ProviderResponse> {
        let _: Option<CallContext> = params_sequence.next().ok(); // ignore CallContext
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
            ProviderResponsePayloadType::ChallengeError => {
                let external_provider_error: Result<ExternalProviderError, CallError> =
                    params_sequence.next();

                if let Ok(r) = external_provider_error {
                    return Some(ProviderResponse {
                        correlation_id: r.correlation_id,
                        result: ProviderResponsePayload::ChallengeError(r.error),
                    });
                }
            }
            _ => error!("get_provider_response: Unsupported payload type"),
        }

        None
    }

    // <pca>
    // pub fn register(platform_state: &PlatformState, methods: &mut Methods) {
    //     let provider_map = platform_state.open_rpc_state.get_provider_map();
    //     for method_name in provider_map.clone().keys() {
    //         if let Some(provider_set) = provider_map.get(method_name) {
    //             if let Some(attributes) = provider_set.attributes {
    //                 let rpc_module_context = RpcModuleContext::new(platform_state.clone());
    //                 let mut rpc_module = RpcModule::new(rpc_module_context.clone());

    //                 // Register request function
    //                 if let Some(method) = provider_set.request.clone() {
    //                     let request_method =
    //                         FireboltOpenRpcMethod::name_with_lowercase_module(&method.name).leak();

    //                     rpc_module
    //                         .register_async_method(
    //                             request_method,
    //                             move |params, context| async move {
    //                                 let mut params_sequence = params.sequence();
    //                                 let call_context: CallContext = params_sequence.next().unwrap();
    //                                 let request: ListenRequest = params_sequence.next().unwrap();
    //                                 let listening = request.listen;

    //                                 ProviderBroker::register_or_unregister_provider(
    //                                     &context.platform_state,
    //                                     attributes.capability.into(),
    //                                     attributes.method.into(),
    //                                     attributes.event,
    //                                     call_context,
    //                                     request,
    //                                 )
    //                                 .await;

    //                                 Ok(ListenerResponse {
    //                                     listening,
    //                                     event: attributes.event.into(),
    //                                 })
    //                             },
    //                         )
    //                         .unwrap();
    //                 }

    //                 // Register focus function
    //                 if let Some(method) = provider_set.focus.clone() {
    //                     let focus_method =
    //                         FireboltOpenRpcMethod::name_with_lowercase_module(&method.name).leak();

    //                     rpc_module
    //                         .register_async_method(
    //                             focus_method,
    //                             move |params, context| async move {
    //                                 let mut params_sequence = params.sequence();
    //                                 let call_context: CallContext = params_sequence.next().unwrap();
    //                                 let request: FocusRequest = params_sequence.next().unwrap();

    //                                 ProviderBroker::focus(
    //                                     &context.platform_state,
    //                                     call_context,
    //                                     attributes.capability.into(),
    //                                     request,
    //                                 )
    //                                 .await;
    //                                 Ok(None) as RpcResult<Option<()>>
    //                             },
    //                         )
    //                         .unwrap();
    //                 }

    //                 // Register response function
    //                 if let Some(method) = provider_set.response.clone() {
    //                     let response_method =
    //                         FireboltOpenRpcMethod::name_with_lowercase_module(&method.name).leak();

    //                     rpc_module
    //                         .register_async_method(
    //                             response_method,
    //                             move |params, context| async move {
    //                                 let params_sequence = params.sequence();

    //                                 if let Some(provider_response) =
    //                                     ProviderRegistrar::get_provider_response(
    //                                         attributes.response_payload_type.clone(),
    //                                         params_sequence,
    //                                     )
    //                                 {
    //                                     ProviderBroker::provider_response(
    //                                         &context.platform_state,
    //                                         provider_response,
    //                                     )
    //                                     .await;
    //                                 }

    //                                 Ok(None) as RpcResult<Option<()>>
    //                             },
    //                         )
    //                         .unwrap();
    //                 }

    //                 // Register error function
    //                 if let Some(method) = provider_set.error.clone() {
    //                     let error_method = method.name.clone().leak();

    //                     rpc_module
    //                         .register_async_method(
    //                             error_method,
    //                             move |params, context| async move {
    //                                 let params_sequence = params.sequence();

    //                                 if let Some(provider_response) =
    //                                     ProviderRegistrar::get_provider_response(
    //                                         attributes.error_payload_type.clone(),
    //                                         params_sequence,
    //                                     )
    //                                 {
    //                                     ProviderBroker::provider_response(
    //                                         &context.platform_state,
    //                                         provider_response,
    //                                     )
    //                                     .await;
    //                                 }

    //                                 Ok(None) as RpcResult<Option<()>>
    //                             },
    //                         )
    //                         .unwrap();
    //                 }

    //                 //methods.merge(rpc_module.clone()).ok();
    //                 methods
    //                     .merge(register_aliases(platform_state, rpc_module))
    //                     .ok();
    //             }
    //         }
    //     }
    // }

    fn register_method_app_event_listener(
        method_name: &'static str,
        rpc_module: &mut RpcModule<RpcModuleContext>,
    ) {
        println!(
            "*** _DEBUG: register_method_app_event_listener: method_name={}",
            method_name
        );
        info!(
            "register_method_app_event_listener: method_name={}",
            method_name
        );

        rpc_module
            .register_async_method(method_name, move |params, context| async move {
                println!(
                    "*** _DEBUG: App event listener registration: method={}",
                    context.method
                );
                info!("App event listener registration: method={}", context.method);

                let mut params_sequence = params.sequence();
                let call_context: CallContext = params_sequence.next().unwrap();
                let request: ListenRequest = params_sequence.next().unwrap();
                let listen = request.listen;

                AppEvents::add_listener(
                    &context.platform_state,
                    context.method.clone(),
                    call_context,
                    request,
                );
                Ok(ListenerResponse {
                    listening: listen,
                    event: context.method.clone(),
                })
            })
            .unwrap();
    }

    fn register_method_provider(
        method_name: &'static str,
        rpc_module: &mut RpcModule<RpcModuleContext>,
    ) {
        println!(
            "*** _DEBUG: register_method_provider: method_name={}",
            method_name
        );
        info!("register_method_provider: method_name={}", method_name);
        rpc_module
            .register_async_method(method_name, move |params, context| async move {
                println!(
                    "*** _DEBUG: Provider registration: method={}",
                    context.method
                );
                info!("Provider registration: method={}", context.method);

                if let Some(provides) = &context.provider_set.provides {
                    let mut params_sequence = params.sequence();
                    let call_context: CallContext = params_sequence.next().unwrap();
                    let request: ListenRequest = params_sequence.next().unwrap();
                    let listening = request.listen;

                    ProviderBroker::register_or_unregister_provider(
                        &context.platform_state,
                        provides.clone(),
                        context.method.clone(),
                        context.method.clone(),
                        call_context,
                        request,
                    )
                    .await;

                    Ok(ListenerResponse {
                        listening,
                        event: context.method.clone(),
                    })
                } else {
                    Err(Error::Custom("Missing provides attribute".to_string()))
                }
            })
            .unwrap();
    }

    fn register_method_app_event_emitter(
        method_name: &'static str,
        rpc_module: &mut RpcModule<RpcModuleContext>,
    ) {
        println!(
            "*** _DEBUG: register_method_app_event_emitter: method_name={}",
            method_name
        );
        info!(
            "register_method_app_event_emitter: method_name={}",
            method_name
        );

        rpc_module
            .register_async_method(method_name, move |params, context| async move {
                println!("*** _DEBUG: App event emitter: method={}", context.method);
                info!("App event emitter: method={}", context.method);
                if let Some(event) = &context.provider_set.provides_to {
                    let mut params_sequence = params.sequence();
                    let _call_context: CallContext = params_sequence.next().unwrap();
                    let result: Value = params_sequence.next().unwrap();

                    AppEvents::emit(&context.platform_state, event, &result).await;
                }

                Ok(None) as RpcResult<Option<()>>
            })
            .unwrap();
    }

    fn register_method_error(
        method_name: &'static str,
        rpc_module: &mut RpcModule<RpcModuleContext>,
    ) {
        println!(
            "*** _DEBUG: register_method_error: method_name={}",
            method_name
        );
        info!("register_method_error: method_name={}", method_name);

        rpc_module
            .register_async_method(method_name, move |params, context| async move {
                println!("*** _DEBUG: Provider error: method={}", context.method);
                info!("Provider error: method={}", context.method);
                let params_sequence = params.sequence();

                if let Some(attributes) = context.provider_set.attributes {
                    if let Some(provider_response) = ProviderRegistrar::get_provider_response(
                        attributes.error_payload_type.clone(),
                        params_sequence,
                    ) {
                        ProviderBroker::provider_response(
                            &context.platform_state,
                            provider_response,
                        )
                        .await;
                    }
                } else {
                    println!(
                        "*** _DEBUG: Provider error: NO ATTRIBUTES: method_name={}",
                        method_name
                    );
                    error!("Provider error: NO ATTRIBUTES: method_name={}", method_name);
                    return Err(Error::Custom(String::from("Missing provider attributes")));
                }

                Ok(None) as RpcResult<Option<()>>
            })
            .unwrap();
    }

    fn register_method_provider_invoker(
        method_name: &'static str,
        rpc_module: &mut RpcModule<RpcModuleContext>,
    ) {
        println!(
            "*** _DEBUG: register_method_provider_invoker: method_name={}",
            method_name
        );
        info!(
            "register_method_provider_invoker: method_name={}",
            method_name
        );

        rpc_module
            .register_async_method(method_name, move |params, context| async move {
                let mut params_sequence = params.sequence();
                let call_context: CallContext = params_sequence.next().unwrap();
                let params: Value = params_sequence.next().unwrap();

                println!("*** _DEBUG: Provider invoker: method={}", context.method);
                info!("Provider invoker: method={}", context.method);

                if let Some(provided_by) = &context.provider_set.provided_by {
                    let provider_map = context.platform_state.open_rpc_state.get_provider_map();

                    if let Some(provided_by_set) = provider_map.get(
                        &FireboltOpenRpcMethod::name_with_lowercase_module(provided_by),
                    ) {
                        if let Some(capability) = &provided_by_set.provides {
                            let (provider_response_payload_tx, provider_response_payload_rx) =
                                oneshot::channel::<ProviderResponsePayload>();

                            let caller = CallerSession {
                                session_id: Some(call_context.session_id.clone()),
                                app_id: Some(call_context.app_id.clone()),
                            };

                            let provider_broker_request = ProviderBrokerRequest {
                                capability: capability.clone(),
                                method: provided_by.clone(),
                                caller,
                                request: ProviderRequestPayload::Generic(params.to_string()),
                                tx: provider_response_payload_tx,
                                app_id: None,
                            };

                            ProviderBroker::invoke_method(
                                &context.platform_state,
                                provider_broker_request,
                            )
                            .await;

                            match timeout(
                                Duration::from_millis(DEFAULT_PROVIDER_RESPONSE_TIMEOUT_MS),
                                provider_response_payload_rx,
                            )
                            .await
                            {
                                Ok(result) => match result {
                                    Ok(provider_response_payload) => {
                                        return Ok(provider_response_payload.as_value());
                                    }
                                    Err(_) => {
                                        return Err(Error::Custom(String::from(
                                            "Error returning from provider",
                                        )));
                                    }
                                },
                                Err(_) => {
                                    return Err(Error::Custom(String::from(
                                        "Provider response timeout",
                                    )));
                                }
                            }
                        }
                    }
                }

                Err(Error::Custom(String::from(
                    "Unexpected schema configuration",
                )))
            })
            .unwrap();
    }

    fn register_method_focus(
        method_name: &'static str,
        rpc_module: &mut RpcModule<RpcModuleContext>,
    ) {
        println!(
            "*** _DEBUG: register_method_focus: method_name={}",
            method_name
        );
        info!("register_method_focus: method_name={}", method_name);

        rpc_module
            .register_async_method(method_name, move |params, context| async move {
                println!("*** _DEBUG: Provider focus: method={}", context.method);
                info!("Provider focus: method={}", context.method);

                if let Some(provides) = &context.provider_set.provides {
                    let mut params_sequence = params.sequence();
                    let call_context: CallContext = params_sequence.next().unwrap();
                    let request: FocusRequest = params_sequence.next().unwrap();

                    ProviderBroker::focus(
                        &context.platform_state,
                        call_context,
                        provides.clone(),
                        request,
                    )
                    .await;

                    Ok(None) as RpcResult<Option<()>>
                } else {
                    Err(Error::Custom("Missing provides attribute".to_string()))
                }
            })
            .unwrap();
    }

    fn register_method_response(
        method_name: &'static str,
        rpc_module: &mut RpcModule<RpcModuleContext>,
    ) {
        println!(
            "*** _DEBUG: register_method_response: method_name={}",
            method_name
        );
        info!("register_method_response: method_name={}", method_name);

        rpc_module
            .register_async_method(method_name, move |params, context| async move {
                println!("*** _DEBUG: Provider response: method={}", context.method);
                info!("Provider response: method={}", context.method);

                let params_sequence = params.sequence();

                if let Some(attributes) = context.provider_set.attributes {
                    if let Some(provider_response) = ProviderRegistrar::get_provider_response(
                        attributes.response_payload_type.clone(),
                        params_sequence,
                    ) {
                        ProviderBroker::provider_response(
                            &context.platform_state,
                            provider_response,
                        )
                        .await;
                    }
                } else {
                    println!(
                        "*** _DEBUG: Provider response: NO ATTRIBUTES: method_name={}",
                        method_name
                    );
                    error!(
                        "Provider response: NO ATTRIBUTES: method_name={}",
                        method_name
                    );
                    return Err(Error::Custom(String::from("Missing provider attributes")));
                }

                Ok(None) as RpcResult<Option<()>>
            })
            .unwrap();
    }

    pub fn register(platform_state: &PlatformState, methods: &mut Methods) {
        let provider_map = platform_state.open_rpc_state.get_provider_map();

        for method_name in provider_map.clone().keys() {
            if let Some(provider_set) = provider_map.get(method_name) {
                let method_name_lcm =
                    FireboltOpenRpcMethod::name_with_lowercase_module(method_name).leak();

                let rpc_module_context = RpcModuleContext::new(
                    platform_state.clone(),
                    method_name_lcm.into(),
                    provider_set.clone(),
                );

                let mut rpc_module = RpcModule::new(rpc_module_context.clone());

                if provider_set.event {
                    if provider_set.provided_by.is_some() {
                        // Register app event listener
                        Self::register_method_app_event_listener(method_name_lcm, &mut rpc_module);
                    } else if provider_set.provides.is_some() || provider_set.provides_to.is_some()
                    {
                        // Register provider
                        Self::register_method_provider(method_name_lcm, &mut rpc_module);
                    }
                } else {
                    if provider_set.provides_to.is_some() {
                        // Register app event emitter
                        Self::register_method_app_event_emitter(method_name_lcm, &mut rpc_module);
                    }

                    if provider_set.error_for.is_some() {
                        // Register error function
                        Self::register_method_error(method_name_lcm, &mut rpc_module);
                    }

                    if provider_set.provided_by.is_some() {
                        // Register provider invoker
                        Self::register_method_provider_invoker(method_name_lcm, &mut rpc_module);
                    }
                }

                if provider_set.focus.is_some() {
                    // Register focus function
                    Self::register_method_focus(method_name_lcm, &mut rpc_module);
                }

                if let Some(method) = provider_set.response.clone() {
                    // Register response function
                    Self::register_method_response(method_name_lcm, &mut rpc_module);
                }

                methods
                    .merge(register_aliases(platform_state, rpc_module))
                    .ok();
            }
        }
    }
    // </pca>
}
