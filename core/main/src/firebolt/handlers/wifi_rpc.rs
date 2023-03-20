use crate::{
    firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState,
    utils::rpc_utils::rpc_err,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::{
    api::{device::{device_info_request::DeviceInfoRequest, device_wifi::{WifiScanRequest, WifiRequest}}, gateway::rpc_gateway_api::CallContext},
    extn::extn_client_message::ExtnResponse,
    log::{error, info, trace},
};

#[rpc(server)]
pub trait Wifi {
    #[method(name = "wifi.scan")]
    async fn scan(&self, ctx: CallContext) -> RpcResult<String>;
}

#[derive(Debug)]
pub struct WifiImpl {
    pub state: PlatformState,
}

#[async_trait]
impl WifiServer for WifiImpl {
    async fn scan(&self, _ctx: CallContext) -> RpcResult<String> {
        if let Ok(response) = self
        .state
        .get_client()
        .send_extn_request(WifiRequest::Scan)
        .await
    {
        if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
            return Ok(v);
        }
    }
    Err(rpc_err("FB error response TBD"))
}

//        Ok("API called".into())
}



pub struct WifiRPCProvider;
impl RippleRPCProvider<WifiImpl> for WifiRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<WifiImpl> {
        (WifiImpl { state }).into_rpc()
    }
}
