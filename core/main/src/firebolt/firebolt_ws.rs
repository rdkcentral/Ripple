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
    service::apps::delegated_launcher_handler::AppManagerState,
    state::{
        cap::permitted_state::PermissionHandler, platform_state::PlatformState,
        session_state::Session,
    },
};
use futures::SinkExt;
use futures::StreamExt;
use jsonrpsee::types::{error::INVALID_REQUEST_CODE, ErrorObject, ErrorResponse, Id};
use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::{
            ApiMessage, ApiProtocol, ClientContext, JsonRpcApiResponse, RpcRequest, RPC_V2,
        },
        observability::log_signal::LogSignal,
    },
    log::{error, info, trace},
    tokio::{
        net::{TcpListener, TcpStream},
        sync::{mpsc, oneshot},
    },
    utils::channel_utils::oneshot_send_and_log,
    uuid::Uuid,
};
use ripple_sdk::{log::debug, tokio};
use tokio_tungstenite::{
    tungstenite::{self, Message},
    WebSocketStream,
};
#[allow(dead_code)]
pub struct FireboltWs {}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ClientIdentity {
    session_id: String,
    app_id: String,
    rpc_v2: bool,
}

struct ConnectionCallbackConfig {
    pub next: oneshot::Sender<ClientIdentity>,
    pub app_state: AppManagerState,
    pub secure: bool,
    pub internal_app_id: Option<String>,
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
        info!("New firebolt connection {:?}", request.uri().query());
        let cfg = self.0;
        let app_id_opt = match cfg.secure {
            true => None,
            false => match get_query(request, "appId", false)? {
                Some(a) => Some(a),
                None => cfg.internal_app_id,
            },
        };
        let session_id = match app_id_opt {
            Some(_) => Uuid::new_v4().to_string(),
            // can unwrap here because if session is not given, then error will be returned
            None => get_query(request, "session", true)?.unwrap(),
        };
        let app_id = match app_id_opt {
            Some(a) => a,
            None => {
                if let Some(app_id) = cfg.app_state.get_app_id_from_session_id(&session_id) {
                    app_id
                } else {
                    let err = tungstenite::http::response::Builder::new()
                        .status(403)
                        .body(Some(format!(
                            "No application session found for {}",
                            session_id
                        )))
                        .unwrap();
                    error!("No application session found for app_id={}", &session_id);
                    return Err(err);
                }
            }
        };

        // If RPCv2 is set as a query param then Ripple will use logic more complicit with the RPCv2 spec.
        // Non-complicit behavior is still available for compatibility with older firebolt clients.
        let rpc_v2 = match get_query(request, "RPCv2", false)? {
            Some(e) => e == "true",
            None => false,
        };

        let cid = ClientIdentity {
            session_id: session_id.clone(),
            app_id,
            rpc_v2,
        };
        oneshot_send_and_log(cfg.next, cid, "ResolveClientIdentity");
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
        Ok(response)
    }
}

impl FireboltWs {
    pub async fn start(
        server_addr: &str,
        state: PlatformState,
        secure: bool,
        internal_app_id: Option<String>,
    ) {
        // Create the event loop and TCP listener we'll accept connections on.
        let try_socket = TcpListener::bind(&server_addr).await; //create the server on the address
        let listener = try_socket.unwrap_or_else(|_| panic!("Failed to bind {:?}", server_addr));
        info!("Listening on: {} secure={}", server_addr, secure);
        let state_for_connection = state.clone();
        let app_state = state.app_manager_state.clone();
        // Let's spawn the handling of each connection in a separate task.
        while let Ok((stream, client_addr)) = listener.accept().await {
            let (connect_tx, connect_rx) = oneshot::channel::<ClientIdentity>();
            let cfg = ConnectionCallbackConfig {
                next: connect_tx,
                app_state: app_state.clone(),
                secure,
                internal_app_id: internal_app_id.clone(),
            };
            match tokio_tungstenite::accept_hdr_async(stream, ConnectionCallback(cfg)).await {
                Err(e) => {
                    error!("websocket connection error {:?}", e);
                }
                Ok(ws_stream) => {
                    trace!("websocket connection success");
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
        info!(
            "Creating new connection_id={} app_id={} session_id={}, gateway_secure={}, port={}",
            connection_id,
            app_id_c,
            session_id_c,
            gateway_secure,
            _client_addr.port()
        );

        let connection_id_c = connection_id.clone();

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
                        let req_text = String::from(msg.to_text().unwrap());
                        let req_id = Uuid::new_v4().to_string();
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
                            if let Some(session) = &state
                                .session_state
                                .get_session_for_connection_id(&connection_id)
                            {
                                let err = ErrorResponse::owned(
                                    ErrorObject::owned::<()>(
                                        INVALID_REQUEST_CODE,
                                        "invalid request".to_owned(),
                                        None,
                                    ),
                                    Id::Null,
                                );

                                let msg = serde_json::to_string(&err).unwrap();
                                let api_msg =
                                    ApiMessage::new(ApiProtocol::JsonRpc, msg, req_id.clone());
                                let _ = session.send_json_rpc(api_msg).await;
                            }
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
