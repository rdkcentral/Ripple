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

use std::{sync::Arc, time::Duration};

use crate::{
    firebolt::rpc::register_aliases,
    service::apps::{
        app_events::AppEvents,
        provider_broker::{ProviderBroker, ProviderBrokerRequest},
    },
    state::{openrpc_state::ProviderRelationSet, platform_state::PlatformState},
};
use jsonrpsee::{
    core::{server::rpc_module::Methods, Error, RpcResult},
    types::{error::CallError, Params, ParamsSequence},
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
                ProviderRequestPayload, ProviderResponse, ProviderResponsePayload,
                ProviderResponsePayloadType,
            },
        },
        gateway::rpc_gateway_api::{CallContext, CallerSession},
    },
    log::{error, info},
    tokio::{sync::oneshot, time::timeout},
};
use serde_json::Value;

// TODO: Add to config
const DEFAULT_PROVIDER_RESPONSE_TIMEOUT_MS: u64 = 15000;

#[derive(Debug)]
enum MethodType {
    AppEventListener,
    Provider,
    AppEventEmitter,
    Error,
    ProviderInvoker,
    Focus,
    Response,
}

#[derive(Clone)]
struct RpcModuleContext {
    platform_state: PlatformState,
    method: String,
    provider_relation_set: ProviderRelationSet,
}

impl RpcModuleContext {
    fn new(
        platform_state: PlatformState,
        method: String,
        provider_relation_set: ProviderRelationSet,
    ) -> Self {
        RpcModuleContext {
            method,
            platform_state,
            provider_relation_set,
        }
    }
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
            ProviderResponsePayloadType::Generic => {
                let external_provider_response: Result<ExternalProviderResponse<Value>, CallError> =
                    params_sequence.next();

                if let Ok(r) = external_provider_response {
                    return Some(ProviderResponse {
                        correlation_id: r.correlation_id,
                        result: ProviderResponsePayload::Generic(r.result),
                    });
                }
            }
            _ => error!("get_provider_response: Unsupported payload type"),
        }

        None
    }

    fn register_method(
        method_name: &'static str,
        method_type: MethodType,
        rpc_module: &mut RpcModule<RpcModuleContext>,
    ) -> bool {
        info!(
            "register_method: method_name={}, method_type={:?}",
            method_name, method_type
        );

        let result = match method_type {
            MethodType::AppEventEmitter => {
                rpc_module.register_async_method(method_name, Self::callback_app_event_emitter)
            }
            MethodType::AppEventListener => {
                rpc_module.register_async_method(method_name, Self::callback_app_event_listener)
            }
            MethodType::Error => {
                rpc_module.register_async_method(method_name, Self::callback_error)
            }
            MethodType::Focus => {
                rpc_module.register_async_method(method_name, Self::callback_focus)
            }
            MethodType::Provider => {
                rpc_module.register_async_method(method_name, Self::callback_register_provider)
            }
            MethodType::ProviderInvoker => {
                rpc_module.register_async_method(method_name, Self::callback_provider_invoker)
            }
            MethodType::Response => {
                rpc_module.register_async_method(method_name, Self::callback_response)
            }
        };

        match result {
            Ok(_) => true,
            Err(e) => {
                error!("register_method: Error registering method: {:?}", e);
                false
            }
        }
    }

    async fn callback_app_event_listener(
        params: Params<'static>,
        context: Arc<RpcModuleContext>,
    ) -> Result<ListenerResponse, Error> {
        info!("callback_app_event_listener: method={}", context.method);

        let mut params_sequence = params.sequence();

        let call_context: CallContext = match params_sequence.next() {
            Ok(context) => context,
            Err(e) => {
                error!("callback_app_event_listener: Error: {:?}", e);
                return Err(Error::Custom("Missing call context".to_string()));
            }
        };

        let request: ListenRequest = match params_sequence.next() {
            Ok(r) => r,
            Err(e) => {
                error!("callback_app_event_listener: Error: {:?}", e);
                return Err(Error::Custom("Missing request".to_string()));
            }
        };

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
    }

    async fn callback_register_provider(
        params: Params<'static>,
        context: Arc<RpcModuleContext>,
    ) -> Result<ListenerResponse, Error> {
        info!("callback_register_provider: method={}", context.method);

        if let Some(capability) = &context.provider_relation_set.capability {
            let mut params_sequence = params.sequence();

            let call_context: CallContext = match params_sequence.next() {
                Ok(context) => context,
                Err(e) => {
                    error!("callback_register_provider: Error: {:?}", e);
                    return Err(Error::Custom("Missing call context".to_string()));
                }
            };

            let request: ListenRequest = match params_sequence.next() {
                Ok(r) => r,
                Err(e) => {
                    error!("callback_register_provider: Error: {:?}", e);
                    return Err(Error::Custom("Missing request".to_string()));
                }
            };

            let listening = request.listen;

            ProviderBroker::register_or_unregister_provider(
                &context.platform_state,
                capability.clone(),
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
    }

    async fn callback_app_event_emitter(
        params: Params<'static>,
        context: Arc<RpcModuleContext>,
    ) -> Result<Option<()>, Error> {
        info!(
            "callback_app_event_emitter: method={}, event={:?}",
            context.method, &context.provider_relation_set.provides_to
        );
        if let Some(event) = &context.provider_relation_set.provides_to {
            let mut params_sequence = params.sequence();
            let _call_context: Option<CallContext> = params_sequence.next().ok();

            let result: Value = match params_sequence.next() {
                Ok(r) => r,
                Err(e) => {
                    error!("callback_app_event_emitter: Error: {:?}", e);
                    return Err(Error::Custom("Missing result".to_string()));
                }
            };

            AppEvents::emit(
                &context.platform_state,
                &FireboltOpenRpcMethod::name_with_lowercase_module(event),
                &result,
            )
            .await;
        }

        Ok(None)
    }

    async fn callback_error(
        params: Params<'static>,
        context: Arc<RpcModuleContext>,
    ) -> Result<Option<()>, Error> {
        info!("callback_error: method={}", context.method);
        let params_sequence = params.sequence();

        if let Some(attributes) = context.provider_relation_set.attributes {
            if let Some(provider_response) = ProviderRegistrar::get_provider_response(
                attributes.error_payload_type.clone(),
                params_sequence,
            ) {
                ProviderBroker::provider_response(&context.platform_state, provider_response).await;
            }
        } else {
            error!(
                "callback_error: NO ATTRIBUTES: context.method={}",
                context.method
            );
            return Err(Error::Custom(String::from("Missing provider attributes")));
        }

        Ok(None) as RpcResult<Option<()>>
    }

    async fn callback_provider_invoker(
        params: Params<'static>,
        context: Arc<RpcModuleContext>,
    ) -> Result<Value, Error> {
        let mut params_sequence = params.sequence();
        let call_context: CallContext = match params_sequence.next() {
            Ok(context) => context,
            Err(e) => {
                error!("callback_provider_invoker: Error: {:?}", e);
                return Err(Error::Custom("Missing call context".to_string()));
            }
        };

        let params: Value = match params_sequence.next() {
            Ok(p) => p,
            Err(e) => {
                error!("callback_provider_invoker: Error: {:?}", e);
                return Err(Error::Custom("Missing params".to_string()));
            }
        };

        info!("callback_provider_invoker: method={}", context.method);

        if let Some(provided_by) = &context.provider_relation_set.provided_by {
            let provider_relation_map = context
                .platform_state
                .open_rpc_state
                .get_provider_relation_map();

            if let Some(provided_by_set) = provider_relation_map.get(
                &FireboltOpenRpcMethod::name_with_lowercase_module(provided_by),
            ) {
                if let Some(capability) = &provided_by_set.capability {
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
                        request: ProviderRequestPayload::Generic(params),
                        tx: provider_response_payload_tx,
                        app_id: None,
                    };

                    ProviderBroker::invoke_method(&context.platform_state, provider_broker_request)
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
                            return Err(Error::Custom(String::from("Provider response timeout")));
                        }
                    }
                }
            }
        }

        Err(Error::Custom(String::from(
            "Unexpected schema configuration",
        )))
    }

    async fn callback_focus(
        params: Params<'static>,
        context: Arc<RpcModuleContext>,
    ) -> Result<Option<()>, Error> {
        info!("callback_focus: method={}", context.method);

        if let Some(capability) = &context.provider_relation_set.capability {
            let mut params_sequence = params.sequence();

            let call_context: CallContext = match params_sequence.next() {
                Ok(context) => context,
                Err(e) => {
                    error!("callback_focus: Error: {:?}", e);
                    return Err(Error::Custom("Missing call context".to_string()));
                }
            };

            let request: FocusRequest = match params_sequence.next() {
                Ok(r) => r,
                Err(e) => {
                    error!("callback_focus: Error: {:?}", e);
                    return Err(Error::Custom("Missing request".to_string()));
                }
            };

            ProviderBroker::focus(
                &context.platform_state,
                call_context,
                capability.clone(),
                request,
            )
            .await;

            Ok(None) as RpcResult<Option<()>>
        } else {
            Err(Error::Custom("Missing provides attribute".to_string()))
        }
    }

    async fn callback_response(
        params: Params<'static>,
        context: Arc<RpcModuleContext>,
    ) -> Result<Option<()>, Error> {
        info!("callback_response: method={}", context.method);

        let params_sequence = params.sequence();

        let response_payload_type = match &context.provider_relation_set.attributes {
            Some(attributes) => attributes.response_payload_type.clone(),
            None => ProviderResponsePayloadType::Generic,
        };

        if let Some(provider_response) =
            ProviderRegistrar::get_provider_response(response_payload_type, params_sequence)
        {
            ProviderBroker::provider_response(&context.platform_state, provider_response).await;
        } else {
            error!(
                "callback_response: Could not resolve response payload type: context.method={}",
                context.method
            );
            return Err(Error::Custom(String::from(
                "Couldn't resolve response payload type",
            )));
        }

        Ok(None)
    }

    pub fn register_methods(platform_state: &PlatformState, methods: &mut Methods) -> u32 {
        let provider_relation_map = platform_state.open_rpc_state.get_provider_relation_map();
        let mut registered_methods = 0;

        for method_name in provider_relation_map.clone().keys() {
            if let Some(provider_relation_set) = provider_relation_map.get(method_name) {
                let mut registered = false;

                let method_name_lcm =
                    FireboltOpenRpcMethod::name_with_lowercase_module(method_name).leak();

                let rpc_module_context = RpcModuleContext::new(
                    platform_state.clone(),
                    method_name_lcm.into(),
                    provider_relation_set.clone(),
                );

                let mut rpc_module = RpcModule::new(rpc_module_context.clone());

                if provider_relation_set.event {
                    if provider_relation_set.provided_by.is_some() {
                        registered = Self::register_method(
                            method_name_lcm,
                            MethodType::AppEventListener,
                            &mut rpc_module,
                        );
                    } else if provider_relation_set.capability.is_some()
                        || provider_relation_set.provides_to.is_some()
                    {
                        registered = Self::register_method(
                            method_name_lcm,
                            MethodType::Provider,
                            &mut rpc_module,
                        );
                    }
                } else if provider_relation_set.provides_to.is_some() {
                    registered = Self::register_method(
                        method_name_lcm,
                        MethodType::AppEventEmitter,
                        &mut rpc_module,
                    );
                } else if provider_relation_set.error_for.is_some() {
                    registered =
                        Self::register_method(method_name_lcm, MethodType::Error, &mut rpc_module);
                } else if provider_relation_set.provided_by.is_some() {
                    registered = Self::register_method(
                        method_name_lcm,
                        MethodType::ProviderInvoker,
                        &mut rpc_module,
                    );
                }

                if !registered {
                    if provider_relation_set.allow_focus_for.is_some() {
                        registered = Self::register_method(
                            method_name_lcm,
                            MethodType::Focus,
                            &mut rpc_module,
                        );
                    } else if provider_relation_set.response_for.is_some() {
                        registered = Self::register_method(
                            method_name_lcm,
                            MethodType::Response,
                            &mut rpc_module,
                        );
                    }
                }

                if registered {
                    methods
                        .merge(register_aliases(platform_state, rpc_module))
                        .ok();

                    registered_methods += 1;
                }
            }
        }

        registered_methods
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{state::openrpc_state::OpenRpcState, utils::test_utils};

    use super::*;
    use jsonrpsee::core::server::rpc_module::Methods;
    use ripple_sdk::tokio;

    #[tokio::test]
    async fn test_register_methods() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None, Vec::new());

        let mut provider_relation_map: HashMap<String, ProviderRelationSet> = HashMap::new();
        provider_relation_map.insert("some.method".to_string(), ProviderRelationSet::new());

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_relation_map(provider_relation_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 0);
    }

    #[tokio::test]
    async fn test_register_method_event_provided_by() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None, Vec::new());

        let provider_relation_set = ProviderRelationSet {
            event: true,
            provided_by: Some("some.other_method".to_string()),
            ..Default::default()
        };

        let mut provider_relation_map: HashMap<String, ProviderRelationSet> = HashMap::new();
        provider_relation_map.insert("some.method".to_string(), provider_relation_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_relation_map(provider_relation_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[tokio::test]
    async fn test_register_method_event_provides() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None, Vec::new());

        let provider_relation_set = ProviderRelationSet {
            event: true,
            capability: Some("some.capability".to_string()),
            ..Default::default()
        };

        let mut provider_relation_map: HashMap<String, ProviderRelationSet> = HashMap::new();
        provider_relation_map.insert("some.method".to_string(), provider_relation_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_relation_map(provider_relation_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[tokio::test]
    async fn test_register_method_event_provides_to() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None, Vec::new());

        let provider_relation_set = ProviderRelationSet {
            event: true,
            provides_to: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_relation_map: HashMap<String, ProviderRelationSet> = HashMap::new();
        provider_relation_map.insert("some.method".to_string(), provider_relation_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_relation_map(provider_relation_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[tokio::test]
    async fn test_register_method_provides_to() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None, Vec::new());

        let provider_relation_set = ProviderRelationSet {
            event: true,
            provides_to: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_relation_map: HashMap<String, ProviderRelationSet> = HashMap::new();
        provider_relation_map.insert("some.method".to_string(), provider_relation_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_relation_map(provider_relation_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[tokio::test]
    async fn test_register_method_error_for() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None, Vec::new());

        let provider_relation_set = ProviderRelationSet {
            error_for: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_relation_map: HashMap<String, ProviderRelationSet> = HashMap::new();
        provider_relation_map.insert("some.method".to_string(), provider_relation_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_relation_map(provider_relation_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[tokio::test]
    async fn test_register_method_provided_by() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None, Vec::new());

        let provider_relation_set = ProviderRelationSet {
            provided_by: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_relation_map: HashMap<String, ProviderRelationSet> = HashMap::new();
        provider_relation_map.insert("some.method".to_string(), provider_relation_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_relation_map(provider_relation_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[tokio::test]
    async fn test_register_method_allow_focus_for() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None, Vec::new());

        let provider_relation_set = ProviderRelationSet {
            allow_focus_for: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_relation_map: HashMap<String, ProviderRelationSet> = HashMap::new();
        provider_relation_map.insert("some.method".to_string(), provider_relation_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_relation_map(provider_relation_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[tokio::test]
    async fn test_register_method_response_for() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None, Vec::new());

        let provider_relation_set = ProviderRelationSet {
            response_for: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_relation_map: HashMap<String, ProviderRelationSet> = HashMap::new();
        provider_relation_map.insert("some.method".to_string(), provider_relation_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_relation_map(provider_relation_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[tokio::test]
    async fn test_register_method_duplicate() {
        const METHOD_NAME: &str = "some.method";

        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None, Vec::new());

        let provider_relation_set = ProviderRelationSet {
            response_for: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let rpc_module_context = RpcModuleContext::new(
            runtime.platform_state.clone(),
            String::from(METHOD_NAME),
            provider_relation_set.clone(),
        );

        let mut rpc_module = RpcModule::new(rpc_module_context.clone());

        let result = ProviderRegistrar::register_method(
            METHOD_NAME,
            MethodType::AppEventEmitter,
            &mut rpc_module,
        );

        assert!(result);

        let result = ProviderRegistrar::register_method(
            METHOD_NAME,
            MethodType::ProviderInvoker,
            &mut rpc_module,
        );

        assert!(!result);
    }
}
