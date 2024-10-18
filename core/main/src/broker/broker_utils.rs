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

use std::time::Duration;

use futures::stream::{SplitSink, SplitStream};
use futures_util::StreamExt;
use ripple_sdk::{
    log::{error, info},
    tokio::{self, net::TcpStream}, utils::rpc_utils::extract_tcp_port,
};
use tokio_tungstenite::{client_async, tungstenite::Message, WebSocketStream};

pub struct BrokerUtils;

impl BrokerUtils {
    pub async fn get_ws_broker(
        endpoint: &str,
        alias: Option<String>,
    ) -> (
        SplitSink<WebSocketStream<TcpStream>, Message>,
        SplitStream<WebSocketStream<TcpStream>>,
    ) {
        info!("Broker Endpoint url {}", endpoint);
        let url_path = if let Some(a) = alias {
            format!("{}{}", endpoint, a)
        } else {
            endpoint.to_owned()
        };
        let url = url::Url::parse(&url_path).unwrap();
        let port = extract_tcp_port(endpoint);
        info!("Url host str {}", url.host_str().unwrap());
        let mut index = 0;

        loop {
            // Try connecting to the tcp port first
            if let Ok(v) = TcpStream::connect(&port).await {
                // Setup handshake for websocket with the tcp port
                // Some WS servers lock on to the Port but not setup handshake till they are fully setup
                if let Ok((stream, _)) = client_async(url_path.clone(), v).await {
                    break stream.split();
                }
            }
            if (index % 10).eq(&0) {
                error!(
                    "Broker with {} failed with retry for last {} secs in {}",
                    url_path, index, port
                );
            }
            index += 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
