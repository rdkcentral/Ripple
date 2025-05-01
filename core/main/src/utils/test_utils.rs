use std::{collections::HashMap, sync::Arc, time::Duration};

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
use hyper::StatusCode;
use ripple_sdk::log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use ripple_sdk::{
    api::{
        firebolt::fb_capabilities::{
            CapEvent, CapListenRPCRequest, CapabilityRole, FireboltCap, FireboltPermission,
        },
        gateway::rpc_gateway_api::{ApiMessage, CallContext},
    },
    log::{self, debug},
    tokio::{
        self,
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::mpsc::{self, Receiver},
        time::sleep,
    },
    utils::logger::init_logger,
};
use ripple_tdk::utils::test_utils::Mockable;
use std::sync::Mutex;

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
    let session = Session::new(ctx.app_id.clone(), Some(session_tx.clone()));
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
    pub fn get(data: String, delay: Option<u64>) -> Self {
        Self { data, delay }
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

//////////// HTTP MOCK SERVER /////////////////////

pub async fn handle_http_connection(
    stream: &mut TcpStream,
    responses: Arc<HashMap<String, MockHttpResponse>>,
) {
    let mut buffer = [0; 1024];
    if let Ok(n) = stream.read(&mut buffer).await {
        // Check if any bytes were read.
        if n > 0 {
            let request = String::from_utf8_lossy(&buffer[..n]);

            if let Some(path) = request.split_whitespace().nth(1) {
                if let Some(response) = responses.get(path) {
                    let status_line = format!("HTTP/1.1 {}\r\n", response.status);

                    let mut headers = String::new();

                    for (key, value) in &response.headers {
                        headers.push_str(&format!("{}: {}\r\n", key, value));
                    }

                    let full_response = format!("{}{}\r\n{}", status_line, headers, response.body);

                    let _ = stream.write_all(full_response.as_bytes()).await;
                } else {
                    let not_found = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(not_found.as_bytes()).await;
                }
            } else {
                let bad_request = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";

                let _ = stream.write_all(bad_request.as_bytes()).await;
            }
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct MockHttpServer {
    port: u16,
    responses: Arc<HashMap<String, MockHttpResponse>>,
}

impl MockHttpServer {
    pub async fn start(responses: HashMap<String, MockHttpResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();
        let shared_responses = Arc::new(responses);
        let shared_listener = Arc::new(listener);
        let shared_responses_clone_for_spawn = shared_responses.clone();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = shared_listener.accept().await {
                let responses_clone = shared_responses_clone_for_spawn.clone();
                tokio::spawn(async move {
                    handle_http_connection(&mut stream, responses_clone).await;
                });
            }
        });

        MockHttpServer {
            port,
            responses: shared_responses.clone(),
        }
    }

    pub fn get_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct MockHttpResponse {
    status: StatusCode,
    headers: HashMap<String, String>,
    body: String,
    delay: Option<u64>,
}

impl MockHttpResponse {
    pub fn get(status: StatusCode, body: String, delay: Option<u64>) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body,
            delay,
        }
    }

    pub fn with_header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);
        self
    }
}

//////////// MOCK LOGGER /////////////////////

#[derive(Clone, Debug)]
pub struct MockLogger {
    messages: Arc<Mutex<Vec<String>>>,
}

impl MockLogger {
    pub fn new() -> Self {
        MockLogger {
            messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn contains(&self, text: &str) -> bool {
        let messages = self.messages.lock().unwrap();
        messages.iter().any(|m| m.contains(text))
    }

    pub fn init() -> Result<Arc<Self>, SetLoggerError> {
        let mock_logger = Arc::new(MockLogger::new());
        let logger_clone_for_setup = Arc::clone(&mock_logger);

        // Leak the Box to make the logger live for 'static.
        let boxed_logger = Box::new(logger_clone_for_setup);
        let static_logger: &'static dyn Log = Box::leak(boxed_logger);

        log::set_logger(static_logger).map(|()| log::set_max_level(LevelFilter::Debug))?;
        Ok(mock_logger)
    }
}

impl Default for MockLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl Log for MockLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &Record) {
        self.messages
            .lock()
            .unwrap()
            .push(format!("{}", record.args()));
    }

    fn flush(&self) {}
}
