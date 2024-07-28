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

use crate::{state::platform_state::PlatformState, utils::rpc_utils::extract_tcp_port};
use futures::stream::{SplitSink, SplitStream};
use futures_util::StreamExt;
use jsonrpsee::{core::RpcResult, types::error::CallError};
use ripple_sdk::{
    api::{
        firebolt::fb_capabilities::CAPABILITY_NOT_AVAILABLE,
        gateway::rpc_gateway_api::{ApiProtocol, CallContext, RpcRequest},
    },
    extn::extn_client_message::ExtnResponse,
    log::{error, info},
    tokio::{self, net::TcpStream},
};
use serde::Deserialize;
use serde_json::from_value;
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
    pub fn rpc_request(method_name: String, call_context: CallContext) -> RpcRequest {
        let mut new_ctx = call_context.clone();
        new_ctx.protocol = ApiProtocol::Extn;
        RpcRequest {
            ctx: new_ctx.clone(),
            method: method_name.clone(),
            params_json: RpcRequest::prepend_ctx(None, &new_ctx),
        }
    }

    pub async fn brokered_rpc_call<T: for<'a> Deserialize<'a>>(
        platform_state: &PlatformState,
        call_context: CallContext,
        method_name: String,
    ) -> RpcResult<T> {
        if let Ok(res) = platform_state
            .get_client()
            .get_extn_client()
            .main_internal_request(Self::rpc_request(method_name.clone(), call_context.clone()))
            .await
        {
            if let Some(ExtnResponse::Value(val)) = res.payload.extract::<ExtnResponse>() {
                if let Ok(v) = from_value::<T>(val) {
                    return Ok(v);
                }
            }
        }

        Err(jsonrpsee::core::Error::Call(CallError::Custom {
            code: CAPABILITY_NOT_AVAILABLE,
            message: format!("the firebolt API {} is not available", method_name),
            data: None,
        }))
    }
}
