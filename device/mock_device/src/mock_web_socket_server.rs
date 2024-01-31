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
use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use http::{HeaderMap, StatusCode};
use ripple_sdk::{
    futures::{stream::SplitSink, SinkExt, StreamExt},
    log::{debug, error, warn},
    tokio::{
        self,
        net::{TcpListener, TcpStream},
        sync::{Mutex, RwLock},
    },
};
use serde_json::{json, Value};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{handshake, Error, Message, Result},
    WebSocketStream,
};

use crate::{
    errors::MockServerWebSocketError,
    mock_data::{json_key, jsonrpc_key, MockData, MockDataError, MockDataKey, MockDataMessage},
    utils::is_value_jsonrpc,
};

#[derive(Clone, Debug, PartialEq)]
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
pub struct MockWebSocketServer {
    mock_data: Arc<RwLock<MockData>>,

    listener: TcpListener,

    conn_path: String,

    conn_headers: HeaderMap,

    conn_query_params: HashMap<String, String>,

    port: u16,

    connected_peer_sinks: Mutex<HashMap<String, SplitSink<WebSocketStream<TcpStream>, Message>>>,
}

impl MockWebSocketServer {
    pub async fn new(
        mock_data: Arc<RwLock<MockData>>,
        server_config: WsServerParameters,
    ) -> Result<Self, MockServerWebSocketError> {
        let listener = Self::create_listener(server_config.port.unwrap_or(0)).await?;
        let port = listener
            .local_addr()
            .map_err(|_| MockServerWebSocketError::CantListen)?
            .port();

        Ok(Self {
            listener,
            mock_data,
            port,
            conn_path: server_config.path.unwrap_or_else(|| "/".to_string()),
            conn_headers: server_config.headers.unwrap_or_else(HeaderMap::new),
            conn_query_params: server_config.query_params.unwrap_or_default(),
            connected_peer_sinks: Mutex::new(HashMap::new()),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    async fn create_listener(port: u16) -> Result<TcpListener, MockServerWebSocketError> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|_| MockServerWebSocketError::CantListen)?;
        debug!("Listening on: {:?}", listener.local_addr().unwrap());

        Ok(listener)
    }

    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }

    pub async fn start_server(self: Arc<Self>) {
        debug!("Waiting for connections");

        while let Ok((stream, peer_addr)) = self.listener.accept().await {
            let server = self.clone();
            tokio::spawn(async move {
                server.accept_connection(peer_addr, stream).await;
            });
        }

        debug!("Shutting down");
    }

    async fn accept_connection(&self, peer: SocketAddr, stream: TcpStream) {
        debug!("Peer address: {}", peer);
        let connection = self.handle_connection(peer, stream).await;

        if let Err(e) = connection {
            match e {
                Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
                err => error!("Error processing connection: {:?}", err),
            }
        }
    }

    async fn handle_connection(&self, peer: SocketAddr, stream: TcpStream) -> Result<()> {
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
        let ws_stream = accept_hdr_async(stream, callback)
            .await
            .expect("Failed to accept");

        let (send, mut recv) = ws_stream.split();

        debug!("New WebSocket connection: {peer}");

        self.add_connected_peer(&peer, send).await;

        while let Some(msg) = recv.next().await {
            let msg = msg?;
            debug!("Message: {:?}", msg);

            if msg.is_close() {
                break;
            }

            if msg.is_text() || msg.is_binary() {
                let msg = msg.to_string();
                let request_message = match serde_json::from_str::<Value>(msg.as_str()).ok() {
                    Some(key) => key,
                    None => {
                        warn!("Request is not valid JSON. Request: {msg}");
                        continue;
                    }
                };

                debug!("Parsed message: {:?}", request_message);

                let responses = match self.find_responses(request_message).await {
                    Some(value) => value,
                    None => continue,
                };

                debug!("Sending responses. resps={responses:?}");

                let mut clients = self.connected_peer_sinks.lock().await;
                let sink = clients.get_mut(&peer.to_string());
                if let Some(sink) = sink {
                    for resp in responses {
                        sink.send(Message::Text(resp.to_string())).await?;
                    }
                } else {
                    error!("No sink found for peer={peer:?}");
                }
            }
        }

        debug!("Connection dropped peer={peer}");
        self.remove_connected_peer(&peer).await;

        Ok(())
    }

    async fn find_responses(&self, request_message: Value) -> Option<Vec<Value>> {
        debug!(
            "is value json rpc {} {}",
            request_message,
            is_value_jsonrpc(&request_message)
        );
        if is_value_jsonrpc(&request_message) {
            let id = request_message
                .get("id")
                .and_then(|s| s.as_u64())
                .unwrap_or(0);

            let key = match jsonrpc_key(&request_message) {
                Ok(key) => key,
                Err(err) => {
                    error!("Request cannot be compared to mock data. {err:?}");
                    return None;
                }
            };

            let responses = self.responses_for_key(key).await.map(|resps| {
                resps.into_iter().map(|mut value| {
                    value.body.as_object_mut().and_then(|obj| obj.insert("id".to_string(), id.into()));
                    value.body
                }).collect()
            })
            .unwrap_or_else(|| vec![json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32600, "message": "Invalid Request"}})]);

            Some(responses)
        } else {
            let key = match json_key(&request_message) {
                Ok(key) => key,
                Err(err) => {
                    error!("Request cannot be compared to mock data. {err:?}");
                    return None;
                }
            };

            let responses = self
                .responses_for_key(key)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|resp| resp.body)
                .collect();

            Some(responses)
        }
    }

    async fn responses_for_key(&self, key: MockDataKey) -> Option<Vec<MockDataMessage>> {
        let mock_data = self.mock_data.read().await;
        debug!("Request received");
        let entry = mock_data.get(&key).cloned();

        entry.map(|(_req, resps)| resps)
    }

    async fn add_connected_peer(
        &self,
        peer: &SocketAddr,
        sink: SplitSink<WebSocketStream<TcpStream>, Message>,
    ) {
        let mut peers = self.connected_peer_sinks.lock().await;
        peers.insert(peer.to_string(), sink);
    }

    async fn remove_connected_peer(&self, peer: &SocketAddr) {
        let mut peers = self.connected_peer_sinks.lock().await;
        let _ = peers.remove(&peer.to_string());
    }

    pub async fn add_request_response(
        &self,
        request: MockDataMessage,
        responses: Vec<MockDataMessage>,
    ) -> Result<(), MockDataError> {
        let key = request.key()?;
        let mut mock_data = self.mock_data.write().await;
        debug!("Adding mock data key={key:?} resps={responses:?}");
        mock_data.insert(key, (request, responses));

        Ok(())
    }

    pub async fn remove_request(&self, request: &MockDataMessage) -> Result<(), MockDataError> {
        let mut mock_data = self.mock_data.write().await;
        let key = request.key()?;
        debug!("Removing mock data key={key:?}");
        let resps = mock_data.remove(&key);
        debug!("Removed mock data responses={resps:?}");

        Ok(())
    }

    pub async fn emit_event(self: Arc<Self>, event: &Value, delay: u32) {
        unimplemented!("Emit event functionality has not yet been implemented {event} {delay}");
        // TODO: handle Results
        // debug!("waiting to send event");

        // let payload = event.clone();

        // tokio::spawn(async move {
        //     tokio::time::sleep(tokio::time::Duration::from_millis(delay.into())).await;

        //     let mut peers = self.connected_peer_sinks.lock().await;
        //     for peer in peers.values_mut() {
        //         debug!("send event to web socket");
        //         let _ = peer.send(Message::Text(payload.to_string())).await;
        //     }
        // });
    }
}

#[cfg(test)]
mod tests {
    use ripple_sdk::tokio::time::{self, error::Elapsed, Duration};

    use super::*;

    async fn start_server(mock_data: MockData) -> Arc<MockWebSocketServer> {
        let mock_data = Arc::new(RwLock::new(mock_data));
        let server = MockWebSocketServer::new(mock_data, WsServerParameters::default())
            .await
            .expect("Unable to start server")
            .into_arc();

        tokio::spawn(server.clone().start_server());

        server
    }

    async fn request_response_with_timeout(
        server: Arc<MockWebSocketServer>,
        request: Message,
    ) -> Result<Option<Result<Message, Error>>, Elapsed> {
        let (client, _) =
            tokio_tungstenite::connect_async(format!("ws://0.0.0.0:{}", server.port()))
                .await
                .expect("Unable to connect to WS server");

        let (mut send, mut receive) = client.split();

        send.send(request).await.expect("Failed to send message");

        time::timeout(Duration::from_secs(1), receive.next()).await
    }

    fn mock_data_json() -> (MockData, Value, Value) {
        let request_body = json!({"key":"value"});
        let request = json!({"type": "json", "body": request_body});
        let response_body = json!({"success": true, "data": "data"});
        let response = json!({"type": "json", "body": response_body});
        let mock_data = HashMap::from([(
            json_key(&request_body).unwrap(),
            (
                (&request).try_into().unwrap(),
                vec![(&response).try_into().unwrap()],
            ),
        )]);

        (mock_data, request_body, response_body)
    }

    fn mock_data_jsonrpc() -> (MockData, Value, Value) {
        let request_body = json!({"jsonrpc":"2.0", "id": 0, "method": "someAction", "params": {}});
        let request = json!({"type": "jsonrpc", "body": request_body});
        let response_body = json!({"jsonrpc": "2.0", "id": 0, "result": {"success": true}});
        let response = json!({"type": "jsonrpc", "body": response_body});
        let mock_data = HashMap::from([(
            jsonrpc_key(&request_body).unwrap(),
            (
                (&request).try_into().unwrap(),
                vec![(&response).try_into().unwrap()],
            ),
        )]);

        (mock_data, request_body, response_body)
    }

    #[test]
    fn test_ws_server_parameters_new() {
        let params = WsServerParameters::new();
        let params_default = WsServerParameters::default();

        assert!(params.headers.is_none());
        assert!(params.path.is_none());
        assert!(params.port.is_none());
        assert!(params.query_params.is_none());
        assert_eq!(params, params_default);
    }

    #[test]
    fn test_ws_server_parameters_props() {
        let mut params = WsServerParameters::new();
        let headers: HeaderMap = {
            let hm = HashMap::from([("Sec-WebSocket-Protocol".to_owned(), "jsonrpc".to_owned())]);
            (&hm).try_into().expect("valid headers")
        };
        let qp = HashMap::from([("appId".to_owned(), "test".to_owned())]);
        params
            .headers(headers.clone())
            .port(16789)
            .path("/some/path")
            .query_params(qp.clone());

        assert_eq!(params.headers, Some(headers));
        assert_eq!(params.port, Some(16789));
        assert_eq!(params.path, Some("/some/path".to_owned()));
        assert_eq!(params.query_params, Some(qp));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_start_server() {
        let mock_data = HashMap::default();
        let server = start_server(mock_data).await;

        let _ = tokio_tungstenite::connect_async(format!("ws://0.0.0.0:{}", server.port()))
            .await
            .expect("Unable to connect to WS server");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_startup_mock_data_json_matched_request() {
        let (mock_data, request_body, response_body) = mock_data_json();
        let server = start_server(mock_data).await;

        let response =
            request_response_with_timeout(server, Message::Text(request_body.to_string()))
                .await
                .expect("no response from server within timeout")
                .expect("connection to server was closed")
                .expect("error in server response");

        assert_eq!(response, Message::Text(response_body.to_string()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_startup_mock_data_json_mismatch_request() {
        let (mock_data, _, _) = mock_data_json();
        let server = start_server(mock_data).await;

        let response = request_response_with_timeout(
            server,
            Message::Text(json!({"key":"value2"}).to_string()),
        )
        .await;

        assert!(response.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_startup_mock_data_jsonrpc_matched_request() {
        let (mock_data, mut request_body, mut response_body) = mock_data_jsonrpc();
        let server = start_server(mock_data).await;

        request_body
            .as_object_mut()
            .and_then(|req| req.insert("id".to_owned(), 327.into()));
        response_body
            .as_object_mut()
            .and_then(|req| req.insert("id".to_owned(), 327.into()));

        let response =
            request_response_with_timeout(server, Message::Text(request_body.to_string()))
                .await
                .expect("no response from server within timeout")
                .expect("connection to server was closed")
                .expect("error in server response");

        assert_eq!(response, Message::Text(response_body.to_string()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_startup_mock_data_jsonrpc_mismatch_request() {
        let (mock_data, _, _) = mock_data_json();
        let server = start_server(mock_data).await;

        let response = request_response_with_timeout(
            server,
            Message::Text(
                json!({"jsonrpc": "2.0", "id": 11, "method": "someUnknownAction"}).to_string(),
            ),
        )
        .await
        .expect("no response from server within timeout")
        .expect("connection to server was closed")
        .expect("error in server response");

        assert_eq!(
            response,
            Message::Text(
                json!({"jsonrpc": "2.0", "id": 11, "error": {"message": "Invalid Request", "code": -32600}})
                    .to_string()
            )
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_startup_mock_data_add_request() {
        let mock_data = HashMap::default();
        let request_body = json!({"key": "value"});
        let response_body = json!({"success": true});
        let server = start_server(mock_data).await;

        server
            .add_request_response(
                (&json!({"type": "json", "body": request_body.clone()}))
                    .try_into()
                    .unwrap(),
                vec![(&json!({"type": "json", "body": response_body.clone()}))
                    .try_into()
                    .unwrap()],
            )
            .await
            .expect("unable to add mock responses");

        let response =
            request_response_with_timeout(server, Message::Text(request_body.to_string()))
                .await
                .expect("no response from server within timeout")
                .expect("connection to server was closed")
                .expect("error in server response");

        assert_eq!(response, Message::Text(response_body.to_string()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_startup_mock_data_remove_request() {
        let mock_data = HashMap::default();
        let request_body = json!({"key": "value"});
        let response_body = json!({"success": true});
        let server = start_server(mock_data).await;
        let request: MockDataMessage = (&json!({"type": "json", "body": request_body.clone()}))
            .try_into()
            .unwrap();

        server
            .add_request_response(
                request.clone(),
                vec![(&json!({"type": "json", "body": response_body.clone()}))
                    .try_into()
                    .unwrap()],
            )
            .await
            .expect("unable to add mock responses");

        server
            .remove_request(&request)
            .await
            .expect("unable to remove request");

        let response =
            request_response_with_timeout(server, Message::Text(request_body.to_string())).await;

        assert!(response.is_err());
    }
}
