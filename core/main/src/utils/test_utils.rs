use std::time::Duration;

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
use futures_util::{SinkExt, StreamExt};
use ripple_sdk::{
    api::{
        firebolt::fb_capabilities::{
            CapEvent, CapListenRPCRequest, CapabilityRole, FireboltCap, FireboltPermission,
        },
        gateway::rpc_gateway_api::{ApiMessage, CallContext},
    },
    log::debug,
    tokio::{
        self,
        net::{TcpListener, TcpStream},
        sync::mpsc::{self, Receiver},
        time::sleep,
    },
    utils::logger::init_logger,
};
use ripple_tdk::utils::test_utils::Mockable;

use crate::state::{
    cap::cap_state::CapState, platform_state::PlatformState, session_state::Session,
};

pub struct MockRuntime {
    pub platform_state: PlatformState,
    pub call_context: CallContext,
}

impl MockRuntime {
    pub fn new() -> Self {
        Self {
            platform_state: PlatformState::mock(),
            call_context: CallContext::mock(),
        }
    }
}

impl Default for MockRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub fn fb_perm(cap: &str, role: Option<CapabilityRole>) -> FireboltPermission {
    FireboltPermission {
        cap: FireboltCap::Full(cap.to_owned()),
        role: role.unwrap_or(CapabilityRole::Use),
    }
}

pub async fn cap_state_listener(
    state: &PlatformState,
    perm: &FireboltPermission,
    cap_event: CapEvent,
) -> Receiver<ApiMessage> {
    let ctx = CallContext::mock();
    let (session_tx, resp_rx) = mpsc::channel(32);
    let session = Session::new(
        ctx.app_id.clone(),
        Some(session_tx.clone()),
        ripple_sdk::api::apps::EffectiveTransport::Websocket,
    );
    state
        .session_state
        .add_session(ctx.session_id.clone(), session);
    CapState::setup_listener(
        state,
        ctx,
        cap_event,
        CapListenRPCRequest {
            capability: perm.cap.as_str(),
            listen: true,
            role: Some(CapabilityRole::Use),
        },
    )
    .await;

    resp_rx
}

pub struct MockCallContext;

impl MockCallContext {
    pub fn get_from_app_id(app_id: &str) -> CallContext {
        CallContext {
            session_id: "session_id".to_owned(),
            request_id: "request_id".to_owned(),
            app_id: app_id.to_owned(),
            call_id: 0,
            protocol: ripple_sdk::api::gateway::rpc_gateway_api::ApiProtocol::JsonRpc,
            method: "some_method".to_owned(),
            cid: Some("cid".to_owned()),
            gateway_secure: false,
            context: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct WSMockData {
    pub data: String,
    pub delay: Option<u64>,
}

impl WSMockData {
    pub fn get(data: String) -> Self {
        Self { data, delay: None }
    }
}

pub struct MockWebsocket;

impl MockWebsocket {
    pub async fn start(
        send_data: Vec<WSMockData>,
        recv_data: Vec<WSMockData>,
        result: mpsc::Sender<bool>,
        on_close: bool,
    ) -> u32 {
        let _ = init_logger("mock websocket tests".to_owned());
        let mut port: u32 = 34743;

        loop {
            let url = format!("127.0.0.1:{}", port);
            match TcpListener::bind(&url).await {
                Ok(l) => {
                    tokio::spawn(async move {
                        if let Ok((stream, _)) = l.accept().await {
                            tokio::spawn(Self::accept_connection(
                                stream, send_data, recv_data, result, on_close,
                            ));
                        }
                    });
                    break;
                }
                Err(_) => port += 1,
            }
        }

        port
    }

    async fn accept_connection(
        stream: TcpStream,
        send_data: Vec<WSMockData>,
        recv_data: Vec<WSMockData>,
        result: mpsc::Sender<bool>,
        on_close: bool,
    ) {
        let addr = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        debug!("Peer address: {}", addr);

        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .expect("Error during the websocket handshake occurred");

        debug!("New WebSocket connection: {}", addr);

        let (mut write, mut read) = ws_stream.split();

        for send in send_data {
            if let Some(d) = send.delay {
                sleep(Duration::from_millis(d)).await;
            }
            write
                .send(tokio_tungstenite::tungstenite::Message::Text(send.data))
                .await
                .unwrap();
            write.flush().await.unwrap();
        }

        if recv_data.is_empty() && on_close {
            while let Some(Ok(v)) = read.next().await {
                if let tokio_tungstenite::tungstenite::Message::Close(_) = v {
                    result.send(true).await.unwrap();
                }
            }
        } else {
            for r in recv_data {
                let value = read.next().await.unwrap().unwrap();
                if let tokio_tungstenite::tungstenite::Message::Text(v) = value {
                    if !r.data.eq_ignore_ascii_case(&v) {
                        result.send(false).await.unwrap();
                        return;
                    }
                } else if let tokio_tungstenite::tungstenite::Message::Close(_) = value {
                    if on_close {
                        result.send(true).await.unwrap();
                    }
                }
            }
        }

        write.close().await.unwrap();
        if !on_close {
            result.send(true).await.unwrap();
        }
    }
}
