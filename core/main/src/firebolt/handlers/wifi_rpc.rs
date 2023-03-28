// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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
use crate::{
    firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState, utils::rpc_utils::rpc_err,
};

use jsonrpsee::{
    core::{async_trait,RpcResult},
    proc_macros::rpc,
    RpcModule,
};

use ripple_sdk::{
    api::{gateway::rpc_gateway_api::CallContext, device::device_wifi::{AccessPointList, WifiRequest}, wifi::WifiResponse},
};

#[rpc(server)]
pub trait  Wifi {
    #[method(name = "wifi.scan")]
    async fn scan(&self, ctx: CallContext) -> RpcResult<AccessPointList>;    
}

#[derive(Debug)]
pub struct WifiImpl {
    pub state: PlatformState,
}

#[async_trait]
impl WifiServer for WifiImpl {
        async fn scan(&self, _ctx: CallContext) -> RpcResult<AccessPointList> {
            if let Ok(response) = self
            .state
            .get_client()
            .send_extn_request(WifiRequest::Scan)
            .await
        {
            if let Some(WifiResponse::WifiScanListResponse(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
        }
        Err(rpc_err("FB error response TBD"))
    }
}


pub struct WifiRPCProvider;
impl RippleRPCProvider<WifiImpl> for WifiRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<WifiImpl> {
        (WifiImpl { state }).into_rpc()
    }
}
