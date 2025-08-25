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
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use super::firebolt_gateway::FireboltGatewayCommand;
use crate::{
    service::apps::delegated_launcher_handler::{AppManagerState, AppManagerState2_0},
    state::{
        cap::permitted_state::PermissionHandler, platform_state::PlatformState,
        session_state::Session,
    },
};
use futures::SinkExt;
use futures::StreamExt;
use jsonrpsee::types::{error::INVALID_REQUEST_CODE, ErrorObject, ErrorResponse, Id};
use ripple_sdk::{
    api::manifest::extn_manifest::ExtnSymbol,
    extn::{
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnResponse},
        extn_id::ExtnId,
    },
    framework::ripple_contract::RippleContract,
    tokio_tungstenite::{
        tungstenite::{self, Message},
        WebSocketStream,
    },
    utils::error::RippleError,
};
use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::{
            ApiMessage, ApiProtocol, ClientContext, JsonRpcApiResponse, RpcRequest, RPC_V2,
        },
        observability::log_signal::LogSignal,
    },
    log::{error, info},
    tokio::{
        net::{TcpListener, TcpStream},
        sync::{mpsc, oneshot, Mutex},
    },
    utils::channel_utils::oneshot_send_and_log,
    uuid::Uuid,
};
use ripple_sdk::{log::debug, tokio};
use ssda_service::ApiGateway;
use ssda_types::gateway::{APIGatewayServiceConnectionDisposition, ApiGatewayServer};
use ssda_types::ServiceId;

#[allow(dead_code)]
pub struct FireboltWs {}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ClientIdentity {
    pub session_id: String,
    pub app_id: String,
    pub rpc_v2: bool,
    pub service_info: Option<ExtnSymbol>,
}
#[derive(Debug, Clone)]
struct ServiceConnection {
    service_id: ServiceId,
}
struct ConnectionCallbackConfig {
    pub next: oneshot::Sender<ClientIdentity>,
    pub app_state: AppManagerState,
    pub app_state2_0: AppManagerState2_0,
    pub app_lifecycle_2_enabled: bool,
    pub secure: bool,
    pub internal_app_id: Option<String>,
    pub extns: Vec<ExtnSymbol>,
    pub service_connection: Arc<std::sync::Mutex<Option<ServiceConnection>>>,
}
impl ConnectionCallbackConfig {
    fn get_extn(&self, id: &str) -> Option<ExtnSymbol> {
        for extn in &self.extns {
            if extn.id.eq(id) {
                return Some(extn.clone());
            }
        }
        None
    }
}
pub struct ConnectionCallback(ConnectionCallbackConfig);

/**
 * Gets a query parameter from the request at the given key.
 * If required=true, then return an error if the param is missing
 *
 *
 * clippy: the `Err`-variant is at least 136 bytes
 * It is not possible to reduce the size of the error message without boxing, and upsetting
 * consumers
 */
#[allow(clippy::result_large_err)]
fn get_query(
    req: &tungstenite::handshake::server::Request,
    key: &'static str,
    required: bool,
) -> Result<Option<String>, tungstenite::handshake::server::ErrorResponse> {
    debug!("get_query: key={}", key);
    let found_q = match req.uri().query() {
        Some(qs) => {
            let qs = querystring::querify(qs);
            qs.iter().find(|q| q.0 == key).cloned()
        }
        None => None,
    };

    if required && found_q.is_none() {
        let err_msg = format!("{} query parameter missing", key);
        error!("{}", err_msg);
        let err = tungstenite::http::response::Builder::new()
            .status(403)
            .body(Some(err_msg))
            .unwrap();
        return Err(err);
    }
    Ok(found_q.map(|q| String::from(q.1)))
}

impl tungstenite::handshake::server::Callback for ConnectionCallback {
    fn on_request(
        self,
        request: &tungstenite::handshake::server::Request,
        mut response: tungstenite::handshake::server::Response,
    ) -> Result<
        tungstenite::handshake::server::Response,
        tungstenite::handshake::server::ErrorResponse,
    > {
        let query = request.uri().query();
        info!("New firebolt connection {:?}", query);
        let cfg = self.0;

        if !cfg.secure {
            if let Ok(Some(extn_id)) = get_query(request, "service_handshake", false) {
                info!("Service handshake for extn_id={}", extn_id);
                if let Some(c) = cfg.get_extn(&extn_id) {
                    // valid extn_id
                    info!("New Service connection {:?}", extn_id);
                    let cid = ClientIdentity {
                        session_id: Uuid::new_v4().to_string(),
                        app_id: extn_id,
                        rpc_v2: true,
                        service_info: Some(c),
                    };
                    oneshot_send_and_log(cfg.next, cid, "ResolveClientIdentity");
                    return Ok(response);
                }
                info!("Extn not found for extn_id={} in {:?} ", extn_id, cfg.extns);
                // invalid extn_id
                let err = tungstenite::http::response::Builder::new()
                    .status(403)
                    .body(Some(format!(
                        "Invalid service handshake for extn_id={}",
                        extn_id
                    )))
                    .unwrap();
                error!("Invalid service handshake for extn_id={}", extn_id);
                return Err(err);
            }
        }

        let app_id_opt = match cfg.secure {
            true => None,
            false => match get_query(request, "appId", false)? {
                Some(a) => Some(a),
                None => cfg.internal_app_id,
            },
        };
        if let Ok(APIGatewayServiceConnectionDisposition::Accept(service_id)) =
            ApiGateway::is_apigateway_connection(request.uri())
        {
            let service_connection = ServiceConnection { service_id };
            *cfg.service_connection.lock().unwrap() = Some(service_connection);
        };
        let session_id = match app_id_opt {
            Some(_) => Uuid::new_v4().to_string(),
            // can unwrap here because if session is not given, then error will be returned
            None => get_query(request, "session", true)?.unwrap(),
        };
        let app_id = match app_id_opt {
            Some(a) => a,
            None => {
                let mut app_id = None;
                if cfg.app_lifecycle_2_enabled {
                    let v = cfg.app_state2_0.get_app_id_from_session_id(&session_id);
                    if v.is_some() {
                        app_id = v;
                    }
                }

                if app_id.is_none() {
                    app_id = cfg.app_state.get_app_id_from_session_id(&session_id);
                }

                match app_id {
                    Some(id) => id,
                    None => {
                        error!(
                            "No application session found for session_id = {}",
                            &session_id
                        );
                        let err = tungstenite::http::response::Builder::new()
                            .status(403)
                            .body(Some(format!(
                                "No application session found for {}",
                                session_id
                            )))
                            .unwrap();
                        return Err(err);
                    }
                }
            }
        };

        // If RPCv2 is set as a query param then Ripple will use logic more complicit with the RPCv2 spec.
        // Non-complicit behavior is still available for compatibility with older firebolt clients.
        let rpc_v2 = match get_query(request, "RPCv2", false)? {
            Some(e) => e == "true",
            None => false,
        };
        /*
        add Sec-WebSocket-Protocol header to the response to indicate we suport jsonrpc
        this was breaking FCA as it tried to use standard websocket protocol and do the upgrade,
        but ripple was not sending the header
        */
        if request.headers().get("Sec-WebSocket-Protocol").is_some() {
            /*
            jsonrpc is the only answer...
            */
            response.headers_mut().insert(
                "Sec-WebSocket-Protocol",
                tungstenite::http::header::HeaderValue::from_str("jsonrpc").unwrap(),
            );
        }

        info!("{:?} {} is_rpc_v2={}", query, app_id, rpc_v2);

        let cid = ClientIdentity {
            session_id: session_id.clone(),
            app_id,
            rpc_v2,
            service_info: None,
        };
        oneshot_send_and_log(cfg.next, cid, "ResolveClientIdentity");

        Ok(response)
    }
}

impl FireboltWs {
    pub async fn start(
        server_addr: &str,
        state: PlatformState,
        secure: bool,
        internal_app_id: Option<String>,
        service_manager: Arc<Mutex<Box<dyn ApiGatewayServer + Send + Sync>>>,
    ) {
        // Create the event loop and TCP listener we'll accept connections on.
        let try_socket = TcpListener::bind(&server_addr).await; //create the server on the address
        let listener = try_socket.unwrap_or_else(|_| panic!("Failed to bind {:?}", server_addr));
        info!("Listening on: {} secure={}", server_addr, secure);
        let state_for_connection = state.clone();
        let extns = state.extn_manifest.get_all_extns();
        let app_state = state.app_manager_state.clone();
        let app_state2_0 = state.lifecycle2_app_state.clone();
        let app_lifecycle_2_enabled = std::env::var("RIPPLE_LIFECYCLE_2_ENABLED")
            .ok()
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);
        // Let's spawn the handling of each connection in a separate task.
        while let Ok((stream, client_addr)) = listener.accept().await {
            let (connect_tx, connect_rx) = oneshot::channel::<ClientIdentity>();
            let service_manager_clone = Arc::clone(&service_manager);
            let service_connection_state = Arc::new(std::sync::Mutex::new(None));
            let cfg = ConnectionCallbackConfig {
                next: connect_tx,
                app_state: app_state.clone(),
                app_state2_0: app_state2_0.clone(),
                app_lifecycle_2_enabled,
                secure,
                internal_app_id: internal_app_id.clone(),
                extns: extns.clone(),
                service_connection: service_connection_state.clone(),
            };
            match ripple_sdk::tokio_tungstenite::accept_hdr_async(stream, ConnectionCallback(cfg))
                .await
            {
                Err(e) => {
                    error!("websocket connection error {:?}", e);
                }
                Ok(ws_stream) => {
                    // let (mut send , mut recv) = ws_stream.split();;
                    //let c = send;
                    let connection = service_connection_state.clone();
                    let service_connection_state = connection.lock().unwrap().clone();
                    //let service_connections = service_connection_state.clone();

                    match service_connection_state {
                        Some(service_connection) => {
                            let service_connection = service_connection.clone();
                            let service_id = service_connection.service_id.clone();

                            info!("Service connection for service_id={:?}", service_id);
                            tokio::spawn(async move {
                                FireboltWs::handle_service_connection(
                                    service_id,
                                    service_manager_clone,
                                    ws_stream,
                                )
                                .await;
                            });
                        }
                        None => {
                            let state_for_connection_c = state_for_connection.clone();
                            tokio::spawn(async move {
                                FireboltWs::handle_connection(
                                    client_addr,
                                    ws_stream,
                                    connect_rx,
                                    state_for_connection_c.clone(),
                                    secure,
                                )
                                .await;
                            });
                        }
                    }
                }
            }
        }
    }
    async fn handle_service_connection(
        service_id: ServiceId,
        service_manager: Arc<Mutex<Box<dyn ApiGatewayServer + Send + Sync>>>,
        ws_stream: WebSocketStream<TcpStream>,
    ) {
        info!("handle_service_connection");
        {
            let mut guard = service_manager.lock().await;
            guard.service_connect(service_id, ws_stream).await.unwrap();
        }
    }
    #[cfg(feature = "ssda")]
    async fn send_t2_event(state: &PlatformState, payload: String) {
        use ripple_sdk::api::gateway::rpc_gateway_api::CallContext;
        let rpc_request = RpcRequest::new(
            String::from("telemetry.event"),
            serde_json::json!({
                "payload": payload,
            })
            .to_string(),
            CallContext::default(),
        );

        state
            .endpoint_state
            .handle_brokerage(rpc_request, None, None, vec![], None, vec![])
            .await;
    }

    #[cfg(not(feature = "ssda"))]
    async fn handle_connection(
        _client_addr: SocketAddr,
        ws_stream: WebSocketStream<TcpStream>,
        connect_rx: oneshot::Receiver<ClientIdentity>,
        state: PlatformState,
        gateway_secure: bool,
    ) {
        use crate::service::ripple_service::service_controller_state::ServiceControllerState;
        let identity = connect_rx.await.unwrap();

        // Generate a unique connection ID
        let connection_id = Uuid::new_v4().to_string();

        if let Some(symbol) = identity.service_info.clone() {
            // Handle service connection
            ServiceControllerState::handle_service_connection(
                _client_addr,
                ws_stream,
                state,
                identity,
                connection_id,
                symbol,
            )
            .await;
        } else {
            // Handle app connection
            Self::handle_app_connection(
                _client_addr,
                ws_stream,
                state,
                identity,
                connection_id,
                gateway_secure,
            )
            .await;
        }
    }
    #[cfg(feature = "ssda")]
    async fn handle_connection(
        _client_addr: SocketAddr,
        ws_stream: WebSocketStream<TcpStream>,
        connect_rx: oneshot::Receiver<ClientIdentity>,
        state: PlatformState,
        gateway_secure: bool,
    ) {
        let identity = connect_rx.await.unwrap();
        let client = state.get_client();
        let app_id = identity.app_id.clone();
        let (session_tx, mut resp_rx) = mpsc::channel(32);
        let ctx = ClientContext {
            session_id: identity.session_id.clone(),
            app_id: app_id.clone(),
            gateway_secure,
        };
        let session = Session::new(identity.app_id.clone(), Some(session_tx.clone()));
        let app_id_c = app_id.clone();
        let session_id_c = identity.session_id.clone();

        let connection_id = Uuid::new_v4().to_string();

        let connection_id_c = connection_id.clone();
        let mut is_service = false;
        if let Some(symbol) = identity.service_info.clone() {
            info!(
                "Creating new service connection_id={} app_id={} session_id={}, gateway_secure={}, port={}",
                connection_id,
                app_id_c,
                session_id_c,
                gateway_secure,
                _client_addr.port()
            );
            is_service = true;
            let id = session_id_c.clone();
            if let Some(sender) = session.get_sender() {
                // Gateway will probably not necessarily be ready when extensions start
                state.session_state.add_session(id, session.clone());
                client
                    .get_extn_client()
                    .add_sender(app_id_c.clone(), symbol, sender);
            }
        } else {
            info!(
                "Creating new connection_id={} app_id={} session_id={}, gateway_secure={}, port={}",
                connection_id,
                app_id_c,
                session_id_c,
                gateway_secure,
                _client_addr.port()
            );
            let msg = FireboltGatewayCommand::RegisterSession {
                session_id: connection_id.clone(),
                session,
            };
            if let Err(e) = client.send_gateway_command(msg) {
                error!("Error registering the connection {:?}", e);
                return;
            }
            if !gateway_secure
                && PermissionHandler::fetch_and_store(&state, &app_id, false)
                    .await
                    .is_err()
            {
                error!("Couldnt pre cache permissions");
            }
        }
        let mut context = vec![];
        if identity.rpc_v2 {
            context.push(RPC_V2.to_string());
        }

        let rpc_context: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(context));
        let (mut sender, mut receiver) = ws_stream.split();
        let mut platform_state = state.clone();
        let context_clone = ctx.clone();
        let spawn_state = state.clone();

        tokio::spawn(async move {
            while let Some(api_message) = resp_rx.recv().await {
                let send_result = sender
                    .send(Message::Text(api_message.jsonrpc_msg.clone()))
                    .await;
                match send_result {
                    Ok(_) => {
                        if is_service {
                            ripple_sdk::log::trace!(
                                "Sent Service response {}",
                                api_message.jsonrpc_msg
                            );
                            continue;
                        }
                        platform_state
                            .metrics
                            .update_api_stage(&api_message.request_id, "response");

                        LogSignal::new(
                            "sent_firebolt_response".to_string(),
                            "firebolt message sent".to_string(),
                            context_clone.clone(),
                        )
                        .with_diagnostic_context_item("cid", &connection_id_c.clone())
                        .with_diagnostic_context_item("result", &api_message.jsonrpc_msg.clone())
                        .emit_debug();
                        if let Some(stats) = platform_state
                            .metrics
                            .get_api_stats(&api_message.request_id)
                        {
                            info!(
                                "Sending Firebolt response: {:?},{}",
                                stats.stats_ref,
                                stats.stats.get_total_time()
                            );
                            if let Some(stats_ref) = stats.stats_ref {
                                let split = format!(
                                    "Full Firebolt Split: {:?},{}",
                                    stats_ref.clone(),
                                    stats.stats.get_stage_durations()
                                );
                                Self::send_t2_event(&spawn_state, split).await;
                            }

                            platform_state
                                .metrics
                                .remove_api_stats(&api_message.request_id);
                        }

                        info!(
                            "Sent Firebolt response cid={} msg={}",
                            connection_id_c.clone(),
                            api_message.jsonrpc_msg
                        );
                    }
                    Err(err) => error!("{:?}", err),
                }
            }
            debug!(
                "api msg rx closed {} {} {}",
                app_id_c.clone(),
                session_id_c.clone(),
                connection_id_c.clone()
            );
        });

        let session_id_c = identity.session_id.clone();
        let app_id_c = identity.app_id.clone();
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(msg) => {
                    if msg.is_text() && !msg.is_empty() {
                        debug!("Received JsonRpc Request {}", msg);
                        let req_id = Uuid::new_v4().to_string();
                        let req_text = String::from(msg.to_text().unwrap());
                        if is_service {
                            match ExtnMessage::try_from(req_text.clone()) {
                                Ok(v) => {
                                    state.get_client().get_extn_client().handle_message(v);
                                }
                                Err(e) => {
                                    error!("failed to parse extn message {:?}", e);
                                    return_invalid_service_error_message(&state, &connection_id, e)
                                        .await;
                                }
                            }
                            continue;
                        }
                        let context = { rpc_context.read().unwrap().clone() };
                        if let Ok(request) = RpcRequest::parse(
                            req_text.clone(),
                            app_id_c.clone(),
                            session_id_c.clone(),
                            req_id.clone(),
                            Some(connection_id.clone()),
                            gateway_secure,
                            context,
                        ) {
                            info!("Received Firebolt request {}", request.params_json);
                            let msg = FireboltGatewayCommand::HandleRpc { request };
                            if let Err(e) = client.clone().send_gateway_command(msg) {
                                error!("failed to send request {:?}", e);
                            }
                        } else if let Some(response) = JsonRpcApiResponse::get_response(&req_text) {
                            let msg = FireboltGatewayCommand::HandleResponse { response };
                            if let Err(e) = client.clone().send_gateway_command(msg) {
                                error!("failed to send request {:?}", e);
                            }
                        } else {
                            return_invalid_format_error_message(req_id, &state, &connection_id)
                                .await;
                            error!("invalid message {}", req_text)
                        }
                    }
                }
                Err(e) => {
                    error!("ws error cid={} error={:?}", connection_id, e);
                }
            }
        }
        debug!("SESSION DEBUG Unregistering {}", connection_id);
        if let Some(symbol) = identity.service_info {
            info!(
                "Unregistering service connection_id={} app_id={} session_id={}",
                connection_id, app_id_c, session_id_c
            );
            client
                .get_extn_client()
                .remove_sender(app_id_c.clone(), symbol);
        }
        let msg = FireboltGatewayCommand::UnregisterSession {
            session_id: identity.session_id.clone(),
            cid: connection_id,
        };
        if let Err(e) = client.send_gateway_command(msg) {
            error!("Error Unregistering {:?}", e);
        }
    }
    #[cfg(not(feature = "ssda"))]
    async fn handle_app_connection(
        _client_addr: SocketAddr,
        ws_stream: WebSocketStream<TcpStream>,
        state: PlatformState,
        identity: ClientIdentity,
        connection_id: String,
        gateway_secure: bool,
    ) {
        info!(
            "Creating new app connection_id={} app_id={} session_id={}, gateway_secure={}, port={}",
            connection_id,
            identity.app_id,
            identity.session_id,
            gateway_secure,
            _client_addr.port()
        );

        let client = state.get_client();
        let app_id = identity.app_id.clone();
        let (session_tx, mut resp_rx) = mpsc::channel(32);
        let ctx = ClientContext {
            session_id: identity.session_id.clone(),
            app_id: app_id.clone(),
            gateway_secure,
        };
        let session = Session::new(identity.app_id.clone(), Some(session_tx.clone()));
        let app_id_c = app_id.clone();
        let session_id_c = identity.session_id.clone();
        let connection_id_c = connection_id.clone();

        let msg = FireboltGatewayCommand::RegisterSession {
            session_id: connection_id.clone(),
            session: session.clone(),
        };
        if let Err(e) = client.send_gateway_command(msg) {
            error!("Error registering the app connection: {:?}", e);
            return;
        }

        if !gateway_secure
            && PermissionHandler::fetch_and_store(&state, &identity.app_id, false)
                .await
                .is_err()
        {
            error!("Couldnt pre cache permissions");
        }

        let mut context = vec![];
        if identity.rpc_v2 {
            context.push(RPC_V2.to_string());
        }

        let rpc_context: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(context));
        let (mut sender, mut receiver) = ws_stream.split();
        let mut platform_state = state.clone();
        let context_clone = ctx.clone();

        tokio::spawn(async move {
            while let Some(api_message) = resp_rx.recv().await {
                let send_result = sender
                    .send(Message::Text(api_message.jsonrpc_msg.clone()))
                    .await;
                match send_result {
                    Ok(_) => {
                        platform_state
                            .metrics
                            .update_api_stage(&api_message.request_id, "response");

                        LogSignal::new(
                            "sent_firebolt_response".to_string(),
                            "firebolt message sent".to_string(),
                            context_clone.clone(),
                        )
                        .with_diagnostic_context_item("cid", &connection_id_c.clone())
                        .with_diagnostic_context_item("result", &api_message.jsonrpc_msg.clone())
                        .emit_debug();
                        if let Some(stats) = platform_state
                            .metrics
                            .get_api_stats(&api_message.request_id)
                        {
                            info!(
                                "Sending Firebolt response: {:?},{}",
                                stats.stats_ref,
                                stats.stats.get_total_time()
                            );
                            debug!(
                                "Full Firebolt Split: {:?},{}",
                                stats.stats_ref,
                                stats.stats.get_stage_durations()
                            );
                            platform_state
                                .metrics
                                .remove_api_stats(&api_message.request_id);
                        }

                        info!(
                            "Sent Firebolt response cid={} msg={}",
                            connection_id_c.clone(),
                            api_message.jsonrpc_msg
                        );
                    }
                    Err(err) => error!("{:?}", err),
                }
            }
            debug!(
                "api msg rx closed {} {} {}",
                app_id_c.clone(),
                session_id_c.clone(),
                connection_id_c.clone()
            );
        });
        let session_id_c = identity.session_id.clone();
        let app_id_c = identity.app_id.clone();
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(msg) => {
                    if msg.is_text() && !msg.is_empty() {
                        debug!("Received JsonRpc Request {}", msg);
                        let req_id = Uuid::new_v4().to_string();
                        let req_text = String::from(msg.to_text().unwrap());
                        let context = { rpc_context.read().unwrap().clone() };
                        if let Ok(request) = RpcRequest::parse(
                            req_text.clone(),
                            app_id_c.clone(),
                            session_id_c.clone(),
                            req_id.clone(),
                            Some(connection_id.clone()),
                            gateway_secure,
                            context,
                        ) {
                            info!("Received Firebolt request {}", request.params_json);
                            let msg = FireboltGatewayCommand::HandleRpc { request };
                            if let Err(e) = client.clone().send_gateway_command(msg) {
                                error!("failed to send request {:?}", e);
                            }
                        } else if let Some(response) = JsonRpcApiResponse::get_response(&req_text) {
                            let msg = FireboltGatewayCommand::HandleResponse { response };
                            if let Err(e) = client.clone().send_gateway_command(msg) {
                                error!("failed to send request {:?}", e);
                            }
                        } else {
                            return_invalid_format_error_message(req_id, &state, &connection_id)
                                .await;
                            error!("invalid message {}", req_text)
                        }
                    }
                }
                Err(e) => {
                    error!("ws error cid={} error={:?}", connection_id, e);
                }
            }
        }
        debug!("SESSION DEBUG Unregistering {}", connection_id);
        let msg = FireboltGatewayCommand::UnregisterSession {
            session_id: identity.session_id.clone(),
            cid: connection_id,
        };
        if let Err(e) = client.send_gateway_command(msg) {
            error!("Error Unregistering {:?}", e);
        }
    }
}
#[cfg(feature = "ssda")]
async fn return_invalid_service_error_message(
    state: &PlatformState,
    connection_id: &str,
    e: RippleError,
) {
    if let Some(session) = state
        .session_state
        .get_session_for_connection_id(connection_id)
    {
        let id = if let RippleError::BrokerError(id) = e.clone() {
            id
        } else {
            Uuid::new_v4().to_string()
        };
        let msg = ExtnMessage {
            id: id.clone(),
            payload: ExtnPayload::Response(ExtnResponse::Error(e)),
            requestor: ExtnId::try_from(session.get_app_id()).unwrap(),
            target: RippleContract::Internal,
            target_id: None,
            ts: None,
        };
        let _ = session.send_json_rpc(msg.into()).await;
    }
}

async fn return_invalid_format_error_message(
    req_id: String,
    state: &PlatformState,
    connection_id: &str,
) {
    if let Some(session) = state
        .session_state
        .get_session_for_connection_id(connection_id)
    {
        let err = ErrorResponse::owned(
            ErrorObject::owned::<()>(INVALID_REQUEST_CODE, "invalid request".to_owned(), None),
            Id::Null,
        );

        let msg = serde_json::to_string(&err).unwrap();
        let api_msg = ApiMessage::new(ApiProtocol::JsonRpc, msg, req_id);
        let _ = session.send_json_rpc(api_msg).await;
    }
}
