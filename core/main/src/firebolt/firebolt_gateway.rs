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

use std::collections::HashMap;

use jsonrpsee::{core::server::rpc_module::Methods, types::TwoPointZero};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_capabilities::JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
            fb_openrpc::FireboltOpenRpcMethod,
        },
        gateway::{
            rpc_error::RpcError,
            rpc_gateway_api::{ApiMessage, ApiProtocol, JsonRpcApiResponse, RpcRequest},
        },
        observability::{log_signal::LogSignal, metrics_util::ApiStats},
    },
    chrono::Utc,
    extn::extn_client_message::ExtnMessage,
    log::{debug, error, info, trace, warn},
    serde_json::{self, Value},
    tokio::{self, runtime::Handle, sync::mpsc::Sender},
};
use serde::{Deserialize, Serialize};

use crate::{
    broker::endpoint_broker::BrokerOutput,
    firebolt::firebolt_gatekeeper::FireboltGatekeeper,
    service::{
        apps::{app_events::AppEvents, provider_broker::ProviderBroker},
        telemetry_builder::TelemetryBuilder,
    },
    state::{
        bootstrap_state::BootstrapState, openrpc_state::OpenRpcState,
        platform_state::PlatformState, session_state::Session,
    },
    utils::router_utils::{capture_stage, get_rpc_header_with_status},
};

use super::rpc_router::RpcRouter;

pub struct FireboltGateway {
    state: BootstrapState,
}

#[derive(Serialize, Deserialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: TwoPointZero,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Serialize, Deserialize)]
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
    HandleResponse {
        response: JsonRpcApiResponse,
    },
    StopServer,
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
        trace!("Starting Gateway Listener");
        let mut firebolt_gateway_rx = self
            .state
            .channels_state
            .get_gateway_receiver()
            .expect("Gateway receiver to be available");
        let handle = Handle::current().metrics();

        while let Some(cmd) = firebolt_gateway_rx.recv().await {
            info!(
                "Current gateway capacity {} {} {}",
                firebolt_gateway_rx.capacity(),
                handle.global_queue_depth(),
                handle.num_alive_tasks()
            );

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
                    ProviderBroker::unregister_session(&self.state.platform_state, cid.clone())
                        .await;
                    self.state
                        .platform_state
                        .endpoint_state
                        .cleanup_for_app(&cid)
                        .await;
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
                HandleResponse { response } => {
                    self.handle_response(response);
                }
                StopServer => {
                    error!("Stopping server");
                    break;
                }
            }
        }
    }

    fn handle_broker_callback(
        platform_state: PlatformState,
        rpc_request: RpcRequest,
    ) -> Sender<BrokerOutput> {
        const REQUESTOR_CALLBACK_TIMEOUT_SECS: u64 = 10;

        let (requestor_callback_tx, mut requestor_callback_rx) =
            tokio::sync::mpsc::channel::<BrokerOutput>(1);

        let start = Utc::now().timestamp_millis();
        let protocol = rpc_request.ctx.protocol.clone();

        tokio::spawn(async move {
            let resp = tokio::time::timeout(
                std::time::Duration::from_secs(REQUESTOR_CALLBACK_TIMEOUT_SECS),
                requestor_callback_rx.recv(),
            )
            .await;

            trace!("handle_broker_callback: resp={:?}", resp);

            match resp {
                Ok(has_broker_output) => {
                    if let Some(broker_output) = has_broker_output {
                        let now = Utc::now().timestamp_millis();

                        let data = match serde_json::to_string(&broker_output.data) {
                            Ok(s) => s,
                            Err(e) => {
                                error!("handle_broker_callback: Could not serialize broker output: e={:?}", e);
                                String::default()
                            }
                        };

                        let mut api_message =
                            ApiMessage::new(protocol, data, rpc_request.ctx.request_id.clone());

                        if let Some(api_stats) = platform_state
                            .metrics
                            .get_api_stats(&rpc_request.ctx.request_id.clone())
                        {
                            api_message.stats = Some(api_stats);
                        }

                        TelemetryBuilder::send_fb_tt(
                            &platform_state,
                            rpc_request,
                            now - start,
                            !broker_output.data.is_error(),
                            &api_message,
                        );
                    }
                }
                Err(e) => error!(
                    "handle_broker_callback: e={:?} request_id={} method={} app_id={}",
                    e, rpc_request.ctx.request_id, rpc_request.ctx.method, rpc_request.ctx.app_id
                ),
            }
        });

        requestor_callback_tx
    }

    pub fn handle_response(&self, response: JsonRpcApiResponse) {
        self.state
            .platform_state
            .endpoint_state
            .handle_broker_response(response);
    }

    pub async fn handle(&self, request: RpcRequest, mut extn_msg: Option<ExtnMessage>) {
        trace!(
            "firebolt_gateway Received Firebolt request {} {} {}",
            request.ctx.request_id,
            request.method,
            request.params_json
        );
        let mut extn_request = false;
        // First check sender if no sender no need to process
        let callback_c = extn_msg.clone();
        LogSignal::new(
            "firebolt_gateway".into(),
            "start_processing_request".into(),
            request.clone(),
        )
        .emit_debug();
        let mut extn_cb = None;
        match request.ctx.protocol {
            ApiProtocol::Extn => {
                extn_request = true;
                // extn protocol with subscription requests means there is no need for callback
                // it is using extn_client::subscribe method which uses id field to resolve.
                if !request.is_subscription()
                    && (callback_c.is_none() || callback_c.unwrap().callback.is_none())
                {
                    trace!("No callback for request {:?} ", request);
                    if let Some(extn_message) = extn_msg.clone() {
                        let extn_id = extn_message.requestor;
                        extn_cb = self
                            .state
                            .platform_state
                            .get_client()
                            .get_extn_client()
                            .get_extn_sender_with_extn_id(&extn_id.to_string());
                    }
                    if extn_cb.is_none() {
                        error!("No sender for request {:?} ", request);
                        return;
                    }
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
        let mut platform_state = self.state.platform_state.clone();

        /*
         * The reason for spawning a new thread is that when request-1 comes, and it waits for
         * user grant. The response from user grant, (eg ChallengeResponse) comes as rpc which
         * in-turn goes through the permission check and sends a gate request. But the single
         * thread which was listening on the channel will be waiting for the user response. This
         * leads to a stall.
         */
        let mut request_c = request.clone();
        request_c.method = FireboltOpenRpcMethod::name_with_lowercase_module(&request.method);

        platform_state
            .metrics
            .add_api_stats(&request_c.ctx.request_id, &request_c.method);

        let metrics_timer = TelemetryBuilder::start_firebolt_metrics_timer(
            &platform_state.get_client().get_extn_client(),
            request_c.method.clone(),
            request_c.ctx.app_id.clone(),
        );
        let fail_open = matches!(
            platform_state
                .get_device_manifest()
                .get_features()
                .intent_validation,
            ripple_sdk::api::manifest::device_manifest::IntentValidation::FailOpen
        );

        let open_rpc_state = self.state.platform_state.open_rpc_state.clone();

        tokio::spawn(async move {
            capture_stage(&platform_state.metrics, &request_c, "context_ready");
            // Validate incoming request parameters.
            if let Err(error_string) = validate_request(open_rpc_state, &request_c, fail_open) {
                TelemetryBuilder::stop_and_send_firebolt_metrics_timer(
                    &platform_state.clone(),
                    metrics_timer,
                    format!("{}", JSON_RPC_STANDARD_ERROR_INVALID_PARAMS),
                )
                .await;

                let json_rpc_error = JsonRpcError {
                    code: JSON_RPC_STANDARD_ERROR_INVALID_PARAMS,
                    message: error_string,
                    data: None,
                };

                send_json_rpc_error(&mut platform_state, &request, json_rpc_error).await;
                return;
            }

            capture_stage(&platform_state.metrics, &request_c, "openrpc_val");

            let result = if extn_request {
                // extn protocol means its an internal Ripple request skip permissions.
                Ok(Vec::new())
            } else {
                FireboltGatekeeper::gate(platform_state.clone(), request_c.clone()).await
            };

            capture_stage(&platform_state.metrics, &request_c, "permission");

            match result {
                Ok(p) => {
                    if let Some(overridden_method) = platform_state
                        .get_manifest()
                        .has_rpc_override_method(&request_c.method)
                    {
                        request_c.method = overridden_method;
                    }

                    if extn_cb.is_some() {
                        if let Some(mut msg) = extn_msg.clone() {
                            msg.callback = extn_cb;
                            let _ = extn_msg.insert(msg);
                        }
                    }

                    let session = if matches!(&request.ctx.protocol, ApiProtocol::JsonRpc) {
                        platform_state
                            .clone()
                            .session_state
                            .get_session(&request_c.ctx)
                    } else {
                        None
                    };

                    let requestor_callback_tx =
                        Self::handle_broker_callback(platform_state.clone(), request_c.clone());

                    let handled = platform_state.endpoint_state.handle_brokerage(
                        request_c.clone(),
                        extn_msg.clone(),
                        None,
                        p,
                        session.clone(),
                        vec![requestor_callback_tx],
                    );

                    if !handled {
                        // Route
                        match request.clone().ctx.protocol {
                            ApiProtocol::Extn => {
                                if let Some(extn_msg) = extn_msg {
                                    LogSignal::new(
                                        "firebolt_gateway".into(),
                                        "routing_to_extn".into(),
                                        request.clone(),
                                    )
                                    .emit_debug();
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
                                if let Some(session) = session {
                                    LogSignal::new(
                                        "firebolt_gateway".into(),
                                        "routing".into(),
                                        request.clone(),
                                    )
                                    .emit_debug();

                                    // if the websocket disconnects before the session is recieved this leads to an error
                                    RpcRouter::route(
                                        platform_state.clone(),
                                        request_c,
                                        session,
                                        metrics_timer.clone(),
                                    )
                                    .await;
                                } else {
                                    error!("session is missing request is not forwarded for request {:?}", request_c.ctx);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let deny_reason = e.reason;
                    // log firebolt response message in RDKTelemetry 1.0 friendly format
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

                    let caps: Vec<String> = e.caps.iter().map(|x| x.as_str()).collect();

                    let json_rpc_error = JsonRpcError {
                        code: deny_reason.get_rpc_error_code(),
                        message: deny_reason.get_rpc_error_message(caps.clone()),
                        data: None,
                    };
                    let caps_diag = caps.join(",");
                    let mut diagnostic_context = HashMap::new();
                    diagnostic_context.insert("reason".to_string(), caps_diag);

                    LogSignal::new(
                        "firebolt_gateway".into(),
                        "denied_no_cap".into(),
                        request.clone(),
                    )
                    .with_diagnostic_context(diagnostic_context)
                    .emit_debug();

                    send_json_rpc_error(&mut platform_state, &request, json_rpc_error).await;
                }
            }
        });
    }
}

fn validate_request(
    open_rpc_state: OpenRpcState,
    request: &RpcRequest,
    fail_open: bool,
) -> Result<(), String> {
    // Existing fail open configuration should work where the
    // call should be delegated to the actual handler
    if fail_open {
        match request.method.to_lowercase().as_str() {
            "lifecyclemanagement.session" | "discovery.launch" => return Ok(()),
            _ => {}
        }
    }

    // Params should be valid given we get the request from Firebolt WS Call context is decorated
    // in index 0
    if let Ok(params) = serde_json::from_str::<Vec<serde_json::Value>>(&request.params_json) {
        // Check if there are params
        let param = params.get(1);
        if param.is_none() {
            // if params are necessary then handler or jq rule will fail
            // This method can unblock other calls.
            return Ok(());
        }
        let param = param.unwrap();
        let method_name = request.method.to_lowercase();

        // Check if the cache is already created using add_json_schema_cache below
        let v = open_rpc_state.validate_schema(&method_name, param);
        if v.is_ok() {
            // Params are valid
            return Ok(());
        } else if let Err(Some(s)) = v {
            // Params are not valid
            return Err(s);
        }
        let major_version = open_rpc_state.get_version().major.to_string();
        let openrpc_validator = open_rpc_state.get_openrpc_validator();
        // Get Method from the validator
        if let Some(rpc_method) = openrpc_validator.get_method(&method_name) {
            // Get schema validator
            let validator = openrpc_validator
                .params_validator(major_version, &rpc_method.name)
                .unwrap();
            // validate
            if let Err(errors) = validator.validate(param) {
                let mut error_string = String::new();
                for error in errors {
                    error_string.push_str(&format!("{} ", error));
                }
                let mut diagnostic_context = HashMap::new();
                diagnostic_context.insert("error".to_string(), error_string.clone());
                LogSignal::new(
                    "firebolt_gateway".into(),
                    "invalid_params".into(),
                    request.clone(),
                )
                .with_diagnostic_context(diagnostic_context)
                .emit_debug();
                return Err(error_string);
            }
            // store validator in runtime for future validations of the same api
            open_rpc_state.add_json_schema_cache(method_name, validator);
        } else {
            // TODO: Currently LifecycleManagement and other APIs are not in the schema. Let these pass through to their
            // respective handlers for now.
            debug!(
                "validate_request: Method not found in schema: {}",
                request.method
            );
        }
    }

    Ok(())
}

async fn send_json_rpc_error(
    platform_state: &mut PlatformState,
    request: &RpcRequest,
    json_rpc_error: JsonRpcError,
) {
    if let Some(session) = platform_state
        .clone()
        .session_state
        .get_session(&request.ctx)
    {
        let status_code = json_rpc_error.code;
        let error_message = JsonRpcMessage {
            jsonrpc: TwoPointZero {},
            id: request.ctx.call_id,
            error: Some(json_rpc_error),
        };

        if let Ok(error_message) = serde_json::to_string(&error_message) {
            let mut api_message = ApiMessage::new(
                request.clone().ctx.protocol,
                error_message,
                request.clone().ctx.request_id,
            );

            if let Some(api_stats) = platform_state
                .metrics
                .get_api_stats(&request.ctx.request_id)
            {
                api_message.stats = Some(ApiStats {
                    api: request.method.clone(),
                    stats_ref: get_rpc_header_with_status(request, status_code),
                    stats: api_stats.stats.clone(),
                });
            }
            platform_state.metrics.update_api_stats_ref(
                &request.ctx.request_id,
                get_rpc_header_with_status(request, status_code),
            );

            if let Err(e) = session.send_json_rpc(api_message).await {
                error!(
                    "send_json_rpc_error: Error sending websocket message: e={:?}",
                    e
                )
            }
        } else {
            error!("send_json_rpc_error: Could not serialize error message");
        }
    } else {
        warn!(
            "send_json_rpc_error: Session no found: method={}",
            request.method
        );
    }
}
