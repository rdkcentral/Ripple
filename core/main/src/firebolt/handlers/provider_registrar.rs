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
    state::{openrpc_state::ProviderSet, platform_state::PlatformState},
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
const DEFAULT_PROVIDER_RESPONSE_TIMEOUT_MS: u64 = 5000;

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
    provider_set: ProviderSet,
}

impl RpcModuleContext {
    fn new(platform_state: PlatformState, method: String, provider_set: ProviderSet) -> Self {
        RpcModuleContext {
            method,
            platform_state,
            provider_set,
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
    }

    async fn callback_register_provider(
        params: Params<'static>,
        context: Arc<RpcModuleContext>,
    ) -> Result<ListenerResponse, Error> {
        info!("callback_register_provider: method={}", context.method);

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
    }

    async fn callback_app_event_emitter(
        params: Params<'static>,
        context: Arc<RpcModuleContext>,
    ) -> Result<Option<()>, Error> {
        info!("callback_app_event_emitter: method={}", context.method);
        if let Some(event) = &context.provider_set.provides_to {
            let mut params_sequence = params.sequence();
            let _call_context: CallContext = params_sequence.next().unwrap();
            let result: Value = params_sequence.next().unwrap();

            AppEvents::emit(&context.platform_state, event, &result).await;
        }

        Ok(None)
    }

    async fn callback_error(
        params: Params<'static>,
        context: Arc<RpcModuleContext>,
    ) -> Result<Option<()>, Error> {
        info!("callback_error: method={}", context.method);
        let params_sequence = params.sequence();

        if let Some(attributes) = context.provider_set.attributes {
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
        // TODO: Is 'Value' correct here? otherwise need to do bespoke return types?
        let mut params_sequence = params.sequence();
        let call_context: CallContext = params_sequence.next().unwrap();
        let params: Value = params_sequence.next().unwrap();

        info!("callback_provider_invoker: method={}", context.method);

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
    }

    async fn callback_response(
        params: Params<'static>,
        context: Arc<RpcModuleContext>,
    ) -> Result<Option<()>, Error> {
        info!("callback_response: method={}", context.method);

        let params_sequence = params.sequence();

        if let Some(attributes) = context.provider_set.attributes {
            if let Some(provider_response) = ProviderRegistrar::get_provider_response(
                attributes.response_payload_type.clone(),
                params_sequence,
            ) {
                ProviderBroker::provider_response(&context.platform_state, provider_response).await;
            }
        } else {
            error!(
                "callback_response: NO ATTRIBUTES: context.method={}",
                context.method
            );
            return Err(Error::Custom(String::from("Missing provider attributes")));
        }

        Ok(None)
    }

    pub fn register_methods(platform_state: &PlatformState, methods: &mut Methods) -> u32 {
        let provider_map = platform_state.open_rpc_state.get_provider_map();
        let mut registered_methods = 0;

        for method_name in provider_map.clone().keys() {
            if let Some(provider_set) = provider_map.get(method_name) {
                let mut registered = false;

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
                        registered = Self::register_method(
                            method_name_lcm,
                            MethodType::AppEventListener,
                            &mut rpc_module,
                        );
                    } else if provider_set.provides.is_some() || provider_set.provides_to.is_some()
                    {
                        registered = Self::register_method(
                            method_name_lcm,
                            MethodType::Provider,
                            &mut rpc_module,
                        );
                    }
                } else if provider_set.provides_to.is_some() {
                    registered = Self::register_method(
                        method_name_lcm,
                        MethodType::AppEventEmitter,
                        &mut rpc_module,
                    );
                } else if provider_set.error_for.is_some() {
                    registered =
                        Self::register_method(method_name_lcm, MethodType::Error, &mut rpc_module);
                } else if provider_set.provided_by.is_some() {
                    registered = Self::register_method(
                        method_name_lcm,
                        MethodType::ProviderInvoker,
                        &mut rpc_module,
                    );
                }

                if !registered {
                    if provider_set.allow_focus_for.is_some() {
                        registered = Self::register_method(
                            method_name_lcm,
                            MethodType::Focus,
                            &mut rpc_module,
                        );
                    } else if provider_set.response_for.is_some() {
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

    #[test]
    fn test_register_methods() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None);

        let mut provider_map: HashMap<String, ProviderSet> = HashMap::new();
        provider_map.insert("some.method".to_string(), ProviderSet::new());

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_map(provider_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 0);
    }

    #[test]
    fn test_register_method_event_provided_by() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None);

        let provider_set = ProviderSet {
            event: true,
            provided_by: Some("some.other_method".to_string()),
            ..Default::default()
        };

        let mut provider_map: HashMap<String, ProviderSet> = HashMap::new();
        provider_map.insert("some.method".to_string(), provider_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_map(provider_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[test]
    fn test_register_method_event_provides() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None);

        let provider_set = ProviderSet {
            event: true,
            provides: Some("some.capability".to_string()),
            ..Default::default()
        };

        let mut provider_map: HashMap<String, ProviderSet> = HashMap::new();
        provider_map.insert("some.method".to_string(), provider_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_map(provider_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[test]
    fn test_register_method_event_provides_to() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None);

        let provider_set = ProviderSet {
            event: true,
            provides_to: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_map: HashMap<String, ProviderSet> = HashMap::new();
        provider_map.insert("some.method".to_string(), provider_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_map(provider_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[test]
    fn test_register_method_provides_to() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None);

        let provider_set = ProviderSet {
            event: true,
            provides_to: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_map: HashMap<String, ProviderSet> = HashMap::new();
        provider_map.insert("some.method".to_string(), provider_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_map(provider_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[test]
    fn test_register_method_error_for() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None);

        let provider_set = ProviderSet {
            error_for: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_map: HashMap<String, ProviderSet> = HashMap::new();
        provider_map.insert("some.method".to_string(), provider_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_map(provider_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[test]
    fn test_register_method_provided_by() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None);

        let provider_set = ProviderSet {
            provided_by: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_map: HashMap<String, ProviderSet> = HashMap::new();
        provider_map.insert("some.method".to_string(), provider_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_map(provider_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[test]
    fn test_register_method_allow_focus_for() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None);

        let provider_set = ProviderSet {
            allow_focus_for: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_map: HashMap<String, ProviderSet> = HashMap::new();
        provider_map.insert("some.method".to_string(), provider_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_map(provider_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[test]
    fn test_register_method_response_for() {
        let mut methods = Methods::new();
        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None);

        let provider_set = ProviderSet {
            response_for: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let mut provider_map: HashMap<String, ProviderSet> = HashMap::new();
        provider_map.insert("some.method".to_string(), provider_set);

        runtime
            .platform_state
            .open_rpc_state
            .set_provider_map(provider_map);

        let registered_methods =
            ProviderRegistrar::register_methods(&runtime.platform_state, &mut methods);

        assert!(registered_methods == 1);
    }

    #[test]
    fn test_register_method_duplicate() {
        const METHOD_NAME: &str = "some.method";

        let mut runtime = test_utils::MockRuntime::new();
        runtime.platform_state.open_rpc_state = OpenRpcState::new(None);

        let provider_set = ProviderSet {
            response_for: Some("some.other.method".to_string()),
            ..Default::default()
        };

        let rpc_module_context = RpcModuleContext::new(
            runtime.platform_state.clone(),
            String::from(METHOD_NAME),
            provider_set.clone(),
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
