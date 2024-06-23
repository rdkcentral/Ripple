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
    types::{error::CallError, Params, ParamsSequence},
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_general::{ListenRequest, ListenerResponse},
            fb_openrpc::FireboltOpenRpcMethod,
            fb_pin::{PinChallengeResponse, PIN_CHALLENGE_CAPABILITY, PIN_CHALLENGE_EVENT},
            provider::{
                self, ChallengeError, ChallengeResponse, ExternalProviderResponse, FocusRequest,
                ProviderResponse, ProviderResponsePayload, ACK_CHALLENGE_CAPABILITY,
                ACK_CHALLENGE_EVENT,
            },
        },
        gateway::rpc_gateway_api::CallContext,
    },
    log::{debug, error},
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Clone, Serialize)]

pub struct ProviderAttributes {
    pub event: &'static str,
    pub response_payload_type: ProviderResponsePayloadType,
    pub error_payload_type: ProviderResponsePayloadType,
    pub capability: &'static str,
    pub method: &'static str,
}

impl ProviderAttributes {
    pub fn get(module: &str) -> Option<&'static ProviderAttributes> {
        println!("*** _DEBUG: ProviderAttributes::get: name={}", module);
        match module {
            "AcknowledgeChallenge" => Some(&ACKNOWLEDGE_CHALLENGE_ATTRIBS),
            "PinChallenge" => Some(&PIN_CHALLENGE_ATTRIBS),
            _ => None,
        }
    }
}

pub const ACKNOWLEDGE_CHALLENGE_ATTRIBS: ProviderAttributes = ProviderAttributes {
    event: ACK_CHALLENGE_EVENT,
    response_payload_type: ProviderResponsePayloadType::ChallengeResponse,
    error_payload_type: ProviderResponsePayloadType::ChallengeError,
    capability: ACK_CHALLENGE_CAPABILITY,
    method: "challenge",
};

pub const PIN_CHALLENGE_ATTRIBS: ProviderAttributes = ProviderAttributes {
    event: PIN_CHALLENGE_EVENT,
    response_payload_type: ProviderResponsePayloadType::PinChallengeResponse,
    error_payload_type: ProviderResponsePayloadType::ChallengeError,
    capability: PIN_CHALLENGE_CAPABILITY,
    method: "challenge",
};

#[derive(Debug, Clone, Serialize)]

pub enum ProviderResponsePayloadType {
    ChallengeResponse,
    ChallengeError,
    PinChallengeResponse,
    KeyboardResult,
    EntityInfoResponse,
    PurchasedContentResponse,
}

impl ToString for ProviderResponsePayloadType {
    fn to_string(&self) -> String {
        match self {
            ProviderResponsePayloadType::ChallengeResponse => "ChallengeResponse".into(),
            ProviderResponsePayloadType::ChallengeError => "ChallengeError".into(),
            ProviderResponsePayloadType::PinChallengeResponse => "PinChallengeResponse".into(),
            ProviderResponsePayloadType::KeyboardResult => "KeyboardResult".into(),
            ProviderResponsePayloadType::EntityInfoResponse => "EntityInfoResponse".into(),
            ProviderResponsePayloadType::PurchasedContentResponse => {
                "PurchasedContentResponse".into()
            }
        }
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
                                move |params, platform_state| async move {
                                    let mut params_sequence = params.sequence();
                                    let call_context: CallContext = params_sequence.next().unwrap();

                                    if let Some(provider_response) =
                                        ProviderRegistrar::get_provider_response(
                                            attributes.response_payload_type.clone(),
                                            params_sequence,
                                        )
                                    {
                                        ProviderBroker::provider_response(
                                            &platform_state,
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
                                move |params, platform_state| async move {
                                    let mut params_sequence = params.sequence();
                                    let call_context: CallContext = params_sequence.next().unwrap();

                                    if let Some(provider_response) =
                                        ProviderRegistrar::get_provider_response(
                                            attributes.error_payload_type.clone(),
                                            params_sequence,
                                        )
                                    {
                                        ProviderBroker::provider_response(
                                            &platform_state,
                                            provider_response,
                                        )
                                        .await;
                                    }

                                    Ok(None) as RpcResult<Option<()>>
                                },
                            )
                            .unwrap();
                    }

                    methods.merge(rpc_module.clone()).ok();
                }
            }
        }
    }
}
