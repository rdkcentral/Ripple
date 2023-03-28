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
    log::info
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
