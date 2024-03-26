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
    accessory::RemoteAccessoryResponse,
    device::device_accessory::{
        AccessoryDeviceListResponse, AccessoryDeviceResponse, AccessoryListRequest,
        AccessoryPairRequest, AccessoryProtocol, RemoteAccessoryRequest,
    },
    gateway::rpc_gateway_api::CallContext,
};

#[rpc(server)]
pub trait Accessory {
    #[method(name = "accessory.list")]
    async fn list(
        &self,
        ctx: CallContext,
        list_request: Option<AccessoryListRequest>,
    ) -> RpcResult<AccessoryDeviceListResponse>;
    #[method(name = "accessory.pair")]
    async fn pair(
        &self,
        ctx: CallContext,
        pair_request: Option<AccessoryPairRequest>,
    ) -> RpcResult<AccessoryDeviceResponse>;
}
pub struct AccessoryImpl {
    pub state: PlatformState,
}

#[async_trait]
impl AccessoryServer for AccessoryImpl {
    async fn list(
        &self,
        _ctx: CallContext,
        list_request_opt: Option<AccessoryListRequest>,
    ) -> RpcResult<AccessoryDeviceListResponse> {
        let list_request = list_request_opt.unwrap_or_default();
        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(RemoteAccessoryRequest::List(list_request))
            .await
        {
            if let Some(RemoteAccessoryResponse::RemoteAccessoryListResponse(value)) =
                response.extract()
            {
                return Ok(value);
            }
        }
        Err(rpc_err("Accessory List error response TBD"))
    }

    async fn pair(
        &self,
        _ctx: CallContext,
        pair_request_opt: Option<AccessoryPairRequest>,
    ) -> RpcResult<AccessoryDeviceResponse> {
        let pair_request = pair_request_opt.unwrap_or_default();
        if pair_request.protocol
            != AccessoryProtocol::get_supported_protocol(self.state.get_device_manifest())
        {
            return Err(rpc_err("Capability Not Supported"));
        }

        if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(RemoteAccessoryRequest::Pair(pair_request))
            .await
        {
            if let Some(RemoteAccessoryResponse::AccessoryPairResponse(v)) = response.extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("Accessory List error response TBD"))
    }
}

pub struct AccessoryRippleProvider;
impl RippleRPCProvider<AccessoryImpl> for AccessoryRippleProvider {
    fn provide(state: PlatformState) -> RpcModule<AccessoryImpl> {
        (AccessoryImpl { state }).into_rpc()
    }
}
