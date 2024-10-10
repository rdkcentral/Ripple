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

use std::{collections::HashMap, net::SocketAddr};

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
use jsonrpsee::types::{error::ErrorCode, ErrorResponse, Id};
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{ApiMessage, ApiProtocol, ClientContext, RpcRequest},
    log::{error, info, trace},
    tokio::{
        net::{TcpListener, TcpStream},
        sync::{mpsc, oneshot},
    },
    utils::{channel_utils::oneshot_send_and_log, error::RippleError},
    uuid::Uuid,
};
use ripple_sdk::{log::debug, tokio};
use fastwebsockets::{upgrade, FragmentCollectorRead};
use fastwebsockets::WebSocketError;
use hyper::service::service_fn;
use hyper::Request;
use hyper::Response;
use http_body_util::Empty;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::server::conn::http1;
#[allow(dead_code)]
pub struct FireboltWs {}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ClientIdentity {
    session_id: String,
    app_id: String,
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
 */

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
            let state_for_connection_c = state_for_connection.clone();
            let internal_app_id_c = internal_app_id.clone();
            tokio::spawn(async move {
                let io = hyper_util::rt::TokioIo::new(stream);
                // let conn_fut = Http::new().serve_connection(io, service_fn(Self::server_upgrade));
                let conn_fut = http1::Builder::new()
                    .serve_connection(io, service_fn(move |req| Self::handle_connection(req, state_for_connection_c.clone(), secure.clone(), internal_app_id_c.clone()))).with_upgrades();
              if let Err(e) = conn_fut.await {
                println!("An error occurred: {:?}", e);
              }
            });
        }
    }

    async fn handle_connection(
        mut req: Request<Incoming>,
        state: PlatformState,
        gateway_secure: bool,
        internal_app_id: Option<String>
      ) -> Result<Response<Empty<Bytes>>, WebSocketError> {
        let (response, fut) = upgrade::upgrade(&mut req)?;
        let uri = req.uri();
        let state_for_connection = state.clone();
        let client = state.get_client();

        // start register connection session
        let (session_tx, mut resp_rx) = mpsc::channel(32);
        let query_segs: Vec<&str> = uri.query().unwrap().split("&").collect::<Vec<&str>>();
        let query_map: HashMap<String, String> = query_segs
            .iter()
            .map(|seg| {
                let kv: Vec<&str> = seg.split("=").collect::<Vec<&str>>();
                (kv[0].to_string(), kv[1].to_string())
            })
            .collect();

        // let app_id = tags.get("app_id").unwrap_or(&"".to_string()).to_string();
        let ctx = ClientContext {
            session_id: query_map.get("sessionId").unwrap_or(&(Uuid::new_v4().to_string()).to_string()).to_string(),
            app_id: query_map.get("appId").unwrap_or(&(Uuid::new_v4().to_string()).to_string()).to_string(),
            gateway_secure: gateway_secure
        };
        let session = Session::new(
            ctx.app_id.clone(),
            Some(session_tx.clone()),
            ripple_sdk::api::apps::EffectiveTransport::Websocket,
        );
        let app_id_c = ctx.app_id.clone();
        let session_id_c = ctx.session_id.clone();

        let connection_id = Uuid::new_v4().to_string();
        info!(
            "Creating new connection_id={} app_id={} session_id={}",
            connection_id, app_id_c, session_id_c
        );
        let connection_id_c = connection_id.clone();

        let msg = FireboltGatewayCommand::RegisterSession {
            session_id: connection_id.clone(),
            session,
        };
        println!("^^^ in fireboltWs handle_connection registering connection");

        if let Err(e) = client.send_gateway_command(msg) {
            error!("Error registering the connection {:?}", e);
            return Err(WebSocketError::ConnectionClosed);
        }
        if !gateway_secure
            && PermissionHandler::fetch_and_store(&state, &app_id_c, false)
                .await
                .is_err()
        {
            error!("Couldnt pre cache permissions");
        }
        //end of register connection session



        tokio::task::spawn(async move {
          if let Err(e) = tokio::task::unconstrained(Self::handle_request(fut, state_for_connection, gateway_secure, resp_rx, ctx, connection_id_c)).await {
            eprintln!("Error in websocket connection: {}", e);
          }
        });
      
        Ok(response)
      }

    async fn handle_request(fut: upgrade::UpgradeFut, state: PlatformState, secure: bool, mut resp_rx: mpsc::Receiver<ApiMessage>, ctx: ClientContext, connection_id: String) -> Result<(), WebSocketError> {
    // let mut ws = fastwebsockets::FragmentCollector::new(fut.await?);
    let ws = fut.await?;
    let (sender, mut receiver) = ws.split(tokio::io::split);
    let mut sender = FragmentCollectorRead::new(sender);

    tokio::spawn(async move {
        while let Some(rs) = resp_rx.recv().await {
            let send_result = receiver.write_frame(fastwebsockets::Frame::text(rs.jsonrpc_msg.into_bytes().to_vec().into())).await;
            match send_result {
                Ok(_) => {
                    debug!(
                        "Sent Firebolt response",
                    );
                }
                Err(err) => error!("{:?}", err),
            }
        }
        // debug!(
        //     "api msg rx closed {} {} {}",
        //     app_id_c, session_id_c, connection_id_c
        // );
    });

    loop {
        let frame = sender.read_frame::<_, WebSocketError>(&mut move |_| async {
                unreachable!();
            })
            .await?;  
        let msg = String::from_utf8(frame.payload.to_vec());
        match msg {
            Ok(msg) => {
                if !msg.is_empty() {
                    //let req_text = String::from(msg.to_text().unwrap());
                    let req_id = Uuid::new_v4().to_string();
                    if let Ok(request) = RpcRequest::parse(
                        msg.clone(),
                        ctx.app_id.clone(),
                        ctx.session_id.clone(),
                        req_id.clone(),
                        Some(connection_id.clone()),
                        ctx.gateway_secure,
                    ) {
                        debug!(
                            "firebolt_ws Received Firebolt request {} {} {}",
                            connection_id, request.ctx.request_id, request.method
                        );
                        let msg = FireboltGatewayCommand::HandleRpc { request };
                        let client = state.get_client();
                        let res = client.clone().send_gateway_command(msg);
                        if let Err(e) = res {
                            error!("failed to send request {:?}", e);
                        }
                    } else {
                        if let Some(session) = &state
                            .session_state
                            .get_session_for_connection_id(&connection_id)
                        {
                            use ripple_sdk::api::apps::EffectiveTransport;
                            let err =
                                ErrorResponse::new(ErrorCode::InvalidRequest.into(), Id::Null);
                            let msg = serde_json::to_string(&err).unwrap();
                            let api_msg =
                                ApiMessage::new(ApiProtocol::JsonRpc, msg, req_id.clone());
                            match session.get_transport() {
                                EffectiveTransport::Bridge(id) => {
                                    let _ = state.send_to_bridge(id, api_msg).await;
                                }
                                EffectiveTransport::Websocket => {
                                    let _ = session.send_json_rpc(api_msg).await;
                                }
                            }
                        }
                        // error!("invalid message {}", req_text)
                    }
                }
            }
            
            Err(e) => {
                // error!("ws error cid={} error={:?}", connection_id, e);
            }
        }
    }    
    Ok(())
    }
}
