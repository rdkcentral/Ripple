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

use jsonrpsee::{core::server::rpc_module::Methods, types::TwoPointZero};
use ripple_sdk::{
    api::{
        apps::EffectiveTransport,
        firebolt::fb_openrpc::FireboltOpenRpcMethod,
        gateway::{
            rpc_error::RpcError,
            rpc_gateway_api::{ApiMessage, ApiProtocol, RpcRequest},
        },
    },
    chrono::Utc,
    extn::extn_client_message::ExtnMessage,
    log::{error, info},
    serde_json::{self, Value},
    tokio,
};
use serde::Serialize;

use crate::{
    firebolt::firebolt_gatekeeper::FireboltGatekeeper,
    service::{apps::app_events::AppEvents, telemetry_builder::TelemetryBuilder},
    state::{bootstrap_state::BootstrapState, session_state::Session},
};

use super::rpc_router::RpcRouter;
pub struct FireboltGateway {
    state: BootstrapState,
}

#[derive(Serialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: TwoPointZero,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone)]
pub enum FireboltGatewayCommand {
    RegisterSession {
        session_id: String,
        session: Session,
    },
    UnregisterSession {
        session_id: String,
        cid: String,
    },
    HandleRpc {
        request: RpcRequest,
    },
    HandleRpcForExtn {
        msg: ExtnMessage,
    },
}

impl FireboltGateway {
    pub fn new(state: BootstrapState, methods: Methods) -> FireboltGateway {
        for method in methods.method_names() {
            info!("Adding RPC method {}", method);
        }
        state.platform_state.router_state.update_methods(methods);
        FireboltGateway { state }
    }

    pub async fn start(&self) {
        info!("Starting Gateway Listener");
        let mut firebolt_gateway_rx = self
            .state
            .channels_state
            .get_gateway_receiver()
            .expect("Gateway receiver to be available");
        while let Some(cmd) = firebolt_gateway_rx.recv().await {
            use FireboltGatewayCommand::*;

            match cmd {
                RegisterSession {
                    session_id,
                    session,
                } => {
                    self.state
                        .platform_state
                        .session_state
                        .add_session(session_id, session);
                }
                UnregisterSession { session_id, cid } => {
                    AppEvents::remove_session(&self.state.platform_state, session_id.clone());
                    self.state.platform_state.session_state.clear_session(&cid);
                }
                HandleRpc { request } => self.handle(request, None).await,
                HandleRpcForExtn { msg } => {
                    if let Some(request) = msg.payload.clone().extract() {
                        self.handle(request, Some(msg)).await
                    } else {
                        error!("Not a valid RPC Request {:?}", msg);
                    }
                }
            }
        }
    }

    pub async fn handle(&self, request: RpcRequest, extn_msg: Option<ExtnMessage>) {
        info!(
            "firebolt_gateway Received Firebolt request {} {} {}",
            request.ctx.request_id, request.method, request.params_json
        );
        // First check sender if no sender no need to process
        let callback_c = extn_msg.clone();
        match request.ctx.protocol {
            ApiProtocol::Extn => {
                if callback_c.is_none() || callback_c.unwrap().callback.is_none() {
                    error!("No callback for request {:?} ", request);
                    return;
                }
            }
            _ => {
                if !self
                    .state
                    .platform_state
                    .session_state
                    .has_session(&request.ctx)
                {
                    error!("No sender for request {:?} ", request);
                    return;
                }
            }
        }
        let platform_state = self.state.platform_state.clone();
        /*
         * The reason for spawning a new thread is that when request-1 comes, and it waits for
         * user grant. The response from user grant, (eg ChallengeResponse) comes as rpc which
         * in-turn goes through the permission check and sends a gate request. But the single
         * thread which was listening on the channel will be waiting for the user response. This
         * leads to a stall.
         */
        let mut request_c = request.clone();
        request_c.method = FireboltOpenRpcMethod::name_with_lowercase_module(&request.method);

        let metrics_timer = TelemetryBuilder::start_firebolt_metrics_timer(
            &platform_state.get_client().get_extn_client(),
            request_c.method.clone(),
            request_c.ctx.app_id.clone(),
        );

        tokio::spawn(async move {
            let start = Utc::now().timestamp_millis();
            let result = FireboltGatekeeper::gate(platform_state.clone(), request_c.clone()).await;

            match result {
                Ok(_) => {
                    // Route
                    match request.clone().ctx.protocol {
                        ApiProtocol::Extn => {
                            if let Some(extn_msg) = extn_msg {
                                RpcRouter::route_extn_protocol(
                                    &platform_state,
                                    request.clone(),
                                    extn_msg,
                                )
                                .await
                            } else {
                                error!("missing invalid message not forwarding");
                            }
                        }
                        _ => {
                            if let Some(session) = platform_state
                                .clone()
                                .session_state
                                .get_session(&request_c.ctx)
                            {
                                // if the websocket disconnects before the session is recieved this leads to an error
                                RpcRouter::route(
                                    platform_state.clone(),
                                    request_c,
                                    session,
                                    metrics_timer.clone(),
                                )
                                .await;
                            } else {
                                error!("session is missing request is not forwarded");
                            }
                        }
                    }
                }
                Err(e) => {
                    let deny_reason = e.reason;
                    // log firebolt response message in RDKTelemetry 1.0 friendly format
                    let now = Utc::now().timestamp_millis();

                    RpcRouter::log_rdk_telemetry_message(
                        &request.ctx.app_id,
                        &request.method,
                        deny_reason.get_rpc_error_code(),
                        now - start,
                    );

                    TelemetryBuilder::stop_and_send_firebolt_metrics_timer(
                        &platform_state.clone(),
                        metrics_timer,
                        format!("{}", deny_reason.get_observability_error_code()),
                    )
                    .await;

                    error!(
                        "Failed gateway present error {:?} {:?}",
                        request, deny_reason
                    );

                    let caps = e.caps.iter().map(|x| x.as_str()).collect();
                    let err = JsonRpcMessage {
                        jsonrpc: TwoPointZero {},
                        id: request.ctx.call_id,
                        error: Some(JsonRpcError {
                            code: deny_reason.get_rpc_error_code(),
                            message: deny_reason.get_rpc_error_message(caps),
                            data: None,
                        }),
                    };

                    let msg = serde_json::to_string(&err).unwrap();

                    let api_msg = ApiMessage::new(
                        request.clone().ctx.protocol,
                        msg,
                        request.clone().ctx.request_id,
                    );
                    if let Some(session) = platform_state
                        .clone()
                        .session_state
                        .get_session(&request.ctx)
                    {
                        match session.get_transport() {
                            EffectiveTransport::Websocket => {
                                if let Err(e) = session.send_json_rpc(api_msg).await {
                                    error!("Error while responding back message {:?}", e)
                                }
                            }
                            EffectiveTransport::Bridge(id) => {
                                if let Err(e) = platform_state.send_to_bridge(id, api_msg).await {
                                    error!("Error while responding back message {:?}", e)
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}
