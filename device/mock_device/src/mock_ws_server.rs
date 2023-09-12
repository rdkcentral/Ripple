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
use std::{collections::HashMap, hash::Hash, net::SocketAddr, sync::Arc};

use http::{HeaderMap, StatusCode};
use ripple_sdk::{
    futures::{SinkExt, StreamExt},
    log::{debug, error},
    tokio::{
        self,
        io::{AsyncRead, AsyncWrite},
        net::TcpListener,
    },
};
use serde_hashkey::{from_key, to_key, Key};
use serde_json::Value;
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{handshake, Error, Message, Result},
};

// pub struct JsonRequest {
//     value: Value,
// }

pub struct WsServerParameters {
    path: Option<String>,

    headers: Option<HeaderMap>,

    query_params: Option<HashMap<String, String>>,

    port: Option<u16>,
}

impl WsServerParameters {
    pub fn new() -> Self {
        Self {
            path: None,
            headers: None,
            query_params: None,
            port: None,
        }
    }
    pub fn path(&mut self, path: &str) -> &mut Self {
        self.path = Some(path.into());

        self
    }
    pub fn headers(&mut self, headers: HeaderMap) -> &mut Self {
        self.headers = Some(headers);

        self
    }
    pub fn query_params(&mut self, query_params: HashMap<String, String>) -> &mut Self {
        self.query_params = Some(query_params);

        self
    }
    pub fn port(&mut self, port: u16) -> &mut Self {
        self.port = Some(port);

        self
    }
}

impl Default for WsServerParameters {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct MockWebsocketServer {
    mock_data: HashMap<Key, Vec<Value>>,

    listener: TcpListener,

    conn_path: String,

    conn_headers: HeaderMap,

    conn_query_params: HashMap<String, String>,

    port: u16,
}

#[derive(Debug)]
pub enum MockWebsocketServerError {
    CantListen,
}

impl MockWebsocketServer {
    pub async fn new(
        mock_data: HashMap<Key, Vec<Value>>,
        server_config: WsServerParameters,
    ) -> Result<Self, MockWebsocketServerError> {
        let listener = Self::create_listener(server_config.port.unwrap_or(0)).await?;
        let port = listener
            .local_addr()
            .map_err(|_| MockWebsocketServerError::CantListen)?
            .port();

        Ok(Self {
            listener,
            mock_data,
            port,
            conn_path: server_config.path.unwrap_or_else(|| "/".to_string()),
            conn_headers: server_config.headers.unwrap_or_else(HeaderMap::new),
            conn_query_params: server_config.query_params.unwrap_or_default(),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    async fn create_listener(port: u16) -> Result<TcpListener, MockWebsocketServerError> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|_| MockWebsocketServerError::CantListen)?;
        debug!("Listening on: {:?}", listener.local_addr().unwrap());

        Ok(listener)
    }

    pub async fn start_server(self: Arc<Self>) {
        debug!("Waiting for connections");

        let server = self.clone();

        while let Ok((stream, peer_addr)) = server.listener.accept().await {
            let s = server.clone();
            tokio::spawn(async move {
                s.accept_connection(peer_addr, stream).await;
            });
        }

        debug!("Shutting down");
    }

    async fn accept_connection<S>(&self, peer: SocketAddr, stream: S)
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        debug!("Peer address: {}", peer);
        let connection = self.handle_connection(peer, stream).await;

        if let Err(e) = connection {
            match e {
                Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
                err => error!("Error processing connection: {:?}", err),
            }
        }
    }

    async fn handle_connection<S>(&self, peer: SocketAddr, stream: S) -> Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let callback = |request: &handshake::client::Request,
                        mut response: handshake::server::Response| {
            let path = request.uri().path();
            if path != self.conn_path {
                *response.status_mut() = StatusCode::NOT_FOUND;
                debug!("Connection response {:?}", response);
            }

            if !self.conn_headers.iter().all(|(header_name, header_value)| {
                request.headers().get(header_name) == Some(header_value)
            }) {
                *response.status_mut() = StatusCode::BAD_REQUEST;
                error!("Incompatible headers. Headers required by server: {:?}. Headers sent in request: {:?}", self.conn_headers, request.headers());
                debug!("Connection response {:?}", response);
            }

            let request_query =
                url::form_urlencoded::parse(request.uri().query().unwrap_or("").as_bytes())
                    .into_owned()
                    .collect::<HashMap<String, String>>();

            let eq_num_params = self.conn_query_params.len() == request_query.len();
            let all_params_match =
                self.conn_query_params
                    .iter()
                    .all(|(param_name, param_value)| {
                        request_query.get(param_name) == Some(param_value)
                    });

            if !(eq_num_params && all_params_match) {
                *response.status_mut() = StatusCode::BAD_REQUEST;
                error!("Incompatible query params. Params required by server: {:?}. Params sent in request: {:?}", self.conn_query_params, request.uri().query());
                debug!("Connection response {:?}", response);
            }

            Ok(response)
        };
        let mut ws_stream = accept_hdr_async(stream, callback)
            .await
            .expect("Failed to accept");

        debug!("New WebSocket connection: {}", peer);

        while let Some(msg) = ws_stream.next().await {
            let msg = msg?;
            debug!("Message: {:?}", msg);

            if msg.is_text() || msg.is_binary() {
                let msg = msg.to_string();
                let request_message = match serde_json::from_str::<Value>(msg.as_str()).ok() {
                    Some(key) => key,
                    None => {
                        error!("Request is not valid JSON. Request: {msg}");
                        continue;
                    }
                };

                debug!("parsed message: {:?}", request_message);
                debug!("key: {:?}", to_key(&request_message).unwrap());

                let response = self.mock_data.get(&to_key(&request_message).unwrap());

                match response {
                    None => error!(
                        "Unrecognised request received. Not responding. Request: {request_message}"
                    ),
                    Some(response_messages) => {
                        for resp in response_messages {
                            ws_stream.send(Message::Text(resp.to_string())).await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn add_request_response(&mut self, request: &Value, responses: Vec<Value>) {
        self.mock_data.insert(to_key(request).unwrap(), responses);
    }

    pub fn remove_request(&mut self, request: &Value) {
        let _ = self.mock_data.remove(&to_key(request).unwrap());
    }
}
