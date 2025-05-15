// Copyright 2025 Comcast Cable Communications Management, LLC
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
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::{
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
    time::{sleep, Duration},
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

/// A trait that defines the behavior of a service in the Application Gateway (AppGW) system.
///
/// The `Service` trait provides a common interface for implementing services that interact
/// with the Application Gateway. It includes methods for handling inbound requests, processing
/// responses, and managing the service's lifecycle, including WebSocket communication.
///
///
/// # Methods
/// - `service_id`: Returns the unique identifier of the service.
/// - `handle_inbound_request`: Processes inbound requests received by the service.
/// - `handle_response`: Handles responses received by the service.
/// - `run`: Manages the service's lifecycle, including WebSocket communication.
///
/// # Example
/// ```rust
/// use async_trait::async_trait;
/// use serde_json::Value;
/// use std::sync::Arc;
/// use tokio::sync::mpsc::{Receiver, Sender};
///
/// use crate::mock_app_gw::service_trait::Service;
///
/// struct MyService {
///     id: String,
/// }
///
/// #[async_trait]
/// impl Service for MyService {
///     fn service_id(&self) -> &str {
///         &self.id
///     }
///
///     async fn handle_inbound_request(&self, request: Value) -> Value {
///         println!("[{}] Handling request: {}", self.service_id(), request);
///         Value::Null // Example response
///     }
///
///     async fn handle_response(&self, response: Value) {
///         println!("[{}] Got response: {}", self.service_id(), response);
///     }
/// }
/// ```
///
/// # Notes
/// - The `run` method is designed to manage the WebSocket connection and handle communication
///   with the Application Gateway. It includes automatic reconnection logic for resilience.
#[async_trait]
pub trait Service: Send + Sync + 'static {
    fn service_id(&self) -> &str;

    async fn handle_inbound_request(&self, request: Value) -> Value;

    async fn handle_response(&self, response: Value) {
        println!("[{}] Got response: {}", self.service_id(), response);
    }

    async fn run(
        self: Arc<Self>,
        url: &str,
        outbound_rx: &mut Receiver<Message>,
        inbound_tx: Sender<Value>,
    ) {
        let ws_stream = establish_connection_to_appgw_with_backoff(url).await;
        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        let register_msg = json!({
            "jsonrpc": "2.0",
            "id": 777, // TBD get a unique ID
            "method": "register",
            "params": { "service_id": self.service_id() }
        });

        // use send_message function to send the register message to AppGW
        if let Err(e) = send_message(&mut ws_tx, Message::Text(register_msg.to_string())).await {
            eprintln!(
                "[{}] Failed to send register message: {}",
                self.service_id(),
                e
            );
            return;
        }

        let sid = self.service_id().to_string();
        let inbound_tx_clone = inbound_tx.clone();
        let self_clone = Arc::clone(&self);

        let sid_c1 = sid.clone();
        let sid_c2 = sid.clone();

        tokio::spawn(async move {
            while let Some(Ok(Message::Text(msg))) = ws_rx.next().await {
                if let Ok(value) = serde_json::from_str::<Value>(&msg) {
                    if value.get("method").is_some() {
                        if let Err(e) = inbound_tx_clone.send(value).await {
                            eprintln!("[{}] Failed to forward inbound: {}", sid_c1, e);
                        }
                    } else if value.get("result").is_some() || value.get("error").is_some() {
                        self_clone.handle_response(value).await;
                    }
                }
            }
            println!("[{}] Connection closed", sid_c1.clone());
            std::process::exit(0);
        });

        while let Some(msg) = outbound_rx.recv().await {
            match msg {
                Message::Close(_) => {
                    println!("[{}] Sending Close message and exiting...", sid_c2);
                    if let Err(e) = send_message(&mut ws_tx, msg).await {
                        eprintln!("[{}] Failed to send Close message: {}", sid_c2, e);
                    }
                    break; // Exit the loop after sending the Close message
                }
                _ => {
                    if let Err(e) = send_message(&mut ws_tx, msg).await {
                        eprintln!("[{}] Failed to send message: {}", sid_c2, e);
                    }
                }
            }
        }
        println!("[{}] Service exiting...", sid);
    }
}

// Utility function for resilient WebSocket connection
async fn establish_connection_to_appgw_with_backoff(
    url: &str,
) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let path = tokio_tungstenite::tungstenite::http::Uri::builder()
        .scheme("ws")
        .authority(url)
        .path_and_query(format!("/?service_handshake={}", 9999))
        .build()
        .unwrap();

    let mut backoff = 1;
    loop {
        match connect_async(path.clone()).await {
            Ok((stream, _)) => {
                println!("[Service] Connected to AppGW at {path}");
                return stream;
            }
            Err(e) => {
                eprintln!(
                    "[Service] Failed to connect to {}: {e}. Retrying in {}s...",
                    path, backoff
                );
                sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(30); // cap wait at 30s
            }
        }
    }
}

/// This function sends a message over the WebSocket connection and flushes the stream.
async fn send_message<S>(ws_tx: &mut S, msg: Message) -> Result<(), Box<dyn std::error::Error>>
where
    S: SinkExt<Message> + Unpin,
    S::Error: std::error::Error + 'static,
{
    ws_tx.feed(msg).await?;
    ws_tx.flush().await?;
    Ok(())
}
