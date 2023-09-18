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

use crate::{
    firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState,
    utils::rpc_utils::rpc_err,
};

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::api::{
    gateway::rpc_gateway_api::CallContext,
    mock_websocket_server::{MockWebsocketServerRequest, MockWebsocketServerResponse},
};

#[rpc(server)]
pub trait MockWebsocketServer {
    #[method(name = "mockwebsocketserver.addRequestResponse")]
    async fn list(
        &self,
        ctx: CallContext,
        add_request_response_request: Option<MockWebsocketServerRequest>,
    ) -> RpcResult<MockWebsocketServerResponse>;
}
pub struct MockWebsocketServerImpl {
    pub state: PlatformState,
}

#[async_trait]
impl MockWebsocketServerServer for MockWebsocketServerImpl {
    async fn list(
        &self,
        _ctx: CallContext,
        list_request_opt: Option<MockWebsocketServerRequest>,
    ) -> RpcResult<MockWebsocketServerResponse> {
        let list_request = list_request_opt.unwrap_or_default();
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(RemoteMockWebsocketServerRequest::List(list_request))
            .await
        {
            if let Some(RemoteMockWebsocketServerResponse::RemoteMockWebsocketServerListResponse(
                value,
            )) = response.payload.extract()
            {
                return Ok(value);
            }
        }
        Err(rpc_err("MockWebsocketServer List error response TBD"))
    }
}

pub struct MockWebsocketServerRippleProvider;

impl RippleRPCProvider<MockWebsocketServerImpl> for MockWebsocketServerRippleProvider {
    fn provide(state: PlatformState) -> RpcModule<MockWebsocketServerImpl> {
        (MockWebsocketServerImpl { state }).into_rpc()
    }
}
