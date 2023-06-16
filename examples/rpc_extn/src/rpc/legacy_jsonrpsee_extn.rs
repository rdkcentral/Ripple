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

use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{ApiProtocol, CallContext, RpcRequest},
    async_trait::async_trait,
    extn::client::extn_client::ExtnClient,
    extn::extn_client_message::ExtnResponse,
};

pub struct LegacyImpl {
    client: ExtnClient,
}

impl LegacyImpl {
    pub fn new(client: ExtnClient) -> LegacyImpl {
        LegacyImpl { client }
    }
}

#[rpc(server)]
pub trait Legacy {
    #[method(name = "legacy.make")]
    fn make(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "legacy.model")]
    async fn model(&self, ctx: CallContext) -> RpcResult<String>;
}

#[async_trait]
impl LegacyServer for LegacyImpl {
    fn make(&self, ctx: CallContext) -> RpcResult<String> {
        let mut client = self.client.clone();
        let mut new_ctx = ctx.clone();
        new_ctx.protocol = ApiProtocol::Extn;

        let rpc_request = RpcRequest {
            ctx: new_ctx.clone(),
            method: "device.make".into(),
            params_json: RpcRequest::prepend_ctx(Some(serde_json::Value::Null), &new_ctx),
        };
        if let Ok(ExtnResponse::Value(v)) = client.request_sync(rpc_request, 5000) {
            if let Some(v) = v.as_str() {
                return Ok(v.into());
            }
        }
        Err(jsonrpsee::core::Error::Custom("Not available".into()))
    }

    async fn model(&self, ctx: CallContext) -> RpcResult<String> {
        let mut client = self.client.clone();
        let mut new_ctx = ctx.clone();
        new_ctx.protocol = ApiProtocol::Extn;

        let rpc_request = RpcRequest {
            ctx: new_ctx.clone(),
            method: "device.model".into(),
            params_json: RpcRequest::prepend_ctx(Some(serde_json::Value::Null), &new_ctx),
        };
        if let Ok(msg) = client.request(rpc_request).await {
            if let Some(ExtnResponse::Value(v)) = msg.payload.clone().extract() {
                if let Some(v) = v.as_str() {
                    return Ok(v.into());
                }
            }
        }
        Err(jsonrpsee::core::Error::Custom("Not available".into()))
    }
}
