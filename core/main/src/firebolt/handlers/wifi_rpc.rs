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
    firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState,
    utils::rpc_utils::rpc_err,
};

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    tracing::info,
    RpcModule,
};

use ripple_sdk::{
    api::{
        device::device_wifi::{AccessPoint, AccessPointList, AccessPointRequest, WifiRequest},
        gateway::rpc_gateway_api::CallContext,
        wifi::{WifiResponse, WifiScanRequestTimeout},
    },
    tokio::time::timeout,
};

use std::time::Duration;

#[rpc(server)]
pub trait Wifi {
    #[method(name = "wifi.scan")]
    async fn scan(&self, ctx: CallContext) -> RpcResult<AccessPointList>;
    #[method(name = "wifi.connect")]
    async fn connect(
        &self,
        ctx: CallContext,
        connect_request: AccessPointRequest,
    ) -> RpcResult<AccessPoint>;
}

#[derive(Debug)]
pub struct WifiImpl {
    pub state: PlatformState,
}

#[async_trait]
impl WifiServer for WifiImpl {
    async fn scan(&self, _ctx: CallContext) -> RpcResult<AccessPointList> {
        let scan_time = WifiScanRequestTimeout::new();
        let client = self.state.get_client();
        let scan_process = client.send_extn_request(WifiRequest::Scan);
        let response = match timeout(Duration::from_secs(scan_time.timeout), scan_process).await {
            Ok(result) => match result {
                Ok(response) => match response.payload.clone().extract() {
                    Some(WifiResponse::WifiScanListResponse(v)) => Ok(v),
                    _ => Err(rpc_err("Wifi scan error response TBD")),
                },
                Err(_) => Err(rpc_err("Wifi scan error response TBD")),
            },
            Err(_) => Err(rpc_err("Wifi scan timed out")),
        };
        response
    }

    async fn connect(
        &self,
        _ctx: CallContext,
        connect_request: AccessPointRequest,
    ) -> RpcResult<AccessPoint> {
        info!("network.connect");
        let scan_time = WifiScanRequestTimeout::new();
        let client = self.state.get_client();
        let scan_process = client.send_extn_request(WifiRequest::Connect(connect_request));

        let response = match timeout(Duration::from_secs(scan_time.timeout), scan_process).await {
            Ok(result) => match result {
                Ok(response) => match response.payload.clone().extract() {
                    Some(WifiResponse::WifiConnectSuccessResponse(v)) => Ok(v),
                    _ => Err(rpc_err("Wifi scan error response TBD")),
                },
                Err(_) => Err(rpc_err("Wifi scan error response TBD")),
            },
            Err(_) => Err(rpc_err("Wifi scan timed out")),
        };
        response
    }
}

pub struct WifiRPCProvider;
impl RippleRPCProvider<WifiImpl> for WifiRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<WifiImpl> {
        (WifiImpl { state }).into_rpc()
    }
}
