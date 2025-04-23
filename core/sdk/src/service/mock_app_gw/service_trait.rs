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

pub const APPGW_WS_URL: &str = "ws://127.0.0.1:1234";

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
        let ws_stream = wait_for_connection(url).await;
        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        let register_msg = json!({
            "jsonrpc": "2.0",
            "method": "register",
            "params": { "service_id": self.service_id() }
        });
        ws_tx
            .send(Message::Text(register_msg.to_string()))
            .await
            .unwrap();

        let sid = self.service_id().to_string();
        let tx_clone = inbound_tx.clone();
        let self_clone = Arc::clone(&self);

        tokio::spawn(async move {
            while let Some(Ok(Message::Text(msg))) = ws_rx.next().await {
                if let Ok(value) = serde_json::from_str::<Value>(&msg) {
                    if value.get("method").is_some() {
                        if let Err(e) = tx_clone.send(value).await {
                            eprintln!("[{}] Failed to forward inbound: {}", sid, e);
                        }
                    } else if value.get("result").is_some() || value.get("error").is_some() {
                        self_clone.handle_response(value).await;
                    }
                }
            }
            println!("[{}] Connection closed", sid);
            std::process::exit(0);
        });

        while let Some(msg) = outbound_rx.recv().await {
            ws_tx.send(msg).await.unwrap();
        }
    }
}

// Utility function for resilient WebSocket connection
async fn wait_for_connection(url: &str) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let mut backoff = 1;
    loop {
        match connect_async(url).await {
            Ok((stream, _)) => {
                println!("[Service] Connected to AppGW at {url}");
                return stream;
            }
            Err(e) => {
                eprintln!(
                    "[Service] Failed to connect to {}: {e}. Retrying in {}s...",
                    url, backoff
                );
                sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(30); // cap wait at 30s
            }
        }
    }
}
