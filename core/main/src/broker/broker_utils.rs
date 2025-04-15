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

use crate::state::platform_state::PlatformState;
use futures::stream::{SplitSink, SplitStream};
use futures_util::StreamExt;
use jsonrpsee::core::RpcResult;
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{CallContext, JsonRpcApiError, RpcRequest},
    log::{error, info},
    tokio::{self, net::TcpStream},
    utils::rpc_utils::extract_tcp_port,
};
use serde_json::Value;
use std::time::Duration;
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
        let tcp_port = port.unwrap();

        info!("Url host str {}", url.host_str().unwrap());
        let mut index = 0;

        loop {
            // Try connecting to the tcp port first
            if let Ok(v) = TcpStream::connect(&tcp_port).await {
                // Setup handshake for websocket with the tcp port
                // Some WS servers lock on to the Port but not setup handshake till they are fully setup
                if let Ok((stream, _)) = client_async(url_path.clone(), v).await {
                    break stream.split();
                }
            }
            if (index % 10).eq(&0) {
                error!(
                    "Broker with {} failed with retry for last {} secs in {}",
                    url_path, index, tcp_port
                );
            }
            index += 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    pub async fn process_for_app_main_request(
        state: &mut PlatformState,
        method: &str,
        params: Option<Value>,
        app_id: &str,
    ) -> RpcResult<Value> {
        let mut rpc_request = RpcRequest::internal(method, None).with_params(params);
        rpc_request.ctx.app_id = app_id.to_owned();
        Self::internal_request(state, rpc_request).await
    }

    pub async fn process_internal_main_request<'a>(
        state: &mut PlatformState,
        method: &'a str,
        params: Option<Value>,
    ) -> RpcResult<Value> {
        Self::process_internal_request(state, None, method, params).await
    }

    pub async fn process_internal_request<'a>(
        state: &mut PlatformState,
        on_behalf_of: Option<CallContext>,
        method: &'a str,
        params: Option<Value>,
    ) -> RpcResult<Value> {
        let rpc_request = RpcRequest::internal(method, on_behalf_of).with_params(params);
        state
            .metrics
            .add_api_stats(&rpc_request.ctx.request_id, method);
        Self::internal_request(state, rpc_request).await
    }

    async fn internal_request(
        state: &mut PlatformState,
        rpc_request: RpcRequest,
    ) -> RpcResult<Value> {
        let method = rpc_request.method.clone();
        match state.internal_rpc_request(&rpc_request).await {
            Ok(res) => match res.as_value() {
                Some(v) => Ok(v),
                None => Err(JsonRpcApiError::default()
                    .with_code(-32100)
                    .with_message(format!("failed to get {} : {:?}", method, res))
                    .into()),
            },
            Err(e) => Err(JsonRpcApiError::default()
                .with_code(-32100)
                .with_message(format!("failed to get {} : {}", method, e))
                .into()),
        }
    }
}
