use std::time::Duration;

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
    api::{device::
        device_wifi::{WifiScanRequest, WifiRequest,AccessPoint, AccessPointList},
        gateway::rpc_gateway_api::CallContext},
    extn::extn_client_message::ExtnResponse,
    log::{error, info, trace}, tokio::time::timeout,
};

#[rpc(server)]
pub trait Wifi {
    #[method(name = "wifi.scan")]
    async fn scan(&self, ctx: CallContext) -> RpcResult<AccessPointList>;
}

#[derive(Debug)]
pub struct WifiImpl {
    pub state: PlatformState,
}

#[async_trait]
impl WifiServer for WifiImpl {
/* 
    async fn scan(&self, _ctx: CallContext) -> RpcResult<AccessPointList> {    
        let scan_req = self
        .state
        .get_client()
        .send_extn_request(WifiRequest::Scan);
        match timeout(Duration::from_secs(5), scan_req).await {
            if let Some(ExtnResponse::WifiScanListResponse(v)) = response.payload.clone().extract() {
                return Ok(v);
            }
    }
}
*/
    
    async fn scan(&self, _ctx: CallContext) -> RpcResult<AccessPointList> {
        if let Ok(response) = self
        .state
        .get_client()
        .send_extn_request(WifiRequest::Scan)
        .await
            {        
                      if let Some(ExtnResponse::String(v)) = response.payload.clone().extract() {
                        let ap_resp = AccessPointList {
                            list: vec![
                            AccessPoint { ssid: (String::from("Test-1")), security_mode: (ripple_sdk::api::device::device_wifi::WifiSecurityMode::None), signal_strength: (0), frequency: (0.0)},
                            AccessPoint { ssid: (String::from("Test-2")), security_mode: (ripple_sdk::api::device::device_wifi::WifiSecurityMode::None), signal_strength: (0), frequency: (0.0)}
                        ] };
                        return Ok(ap_resp);
                     }
                 }    Err(rpc_err("FB error response TBD"))
}

//        Ok("API called".into())
}



pub struct WifiRPCProvider;
impl RippleRPCProvider<WifiImpl> for WifiRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<WifiImpl> {
        (WifiImpl { state }).into_rpc()
    }
}
