use crate::{
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    helpers::ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
    managers::capability_manager::{
        CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
    },
    managers::config_manager::ConfigManager,
    platform_state::PlatformState,
};
use dab::core::message::{DabRequestPayload, DabResponsePayload};
use dab::core::model::wifi::{
    AccessPoint, AccessPointList, AccessPointRequest, DabWifiScanRequest, WifiRequest,
};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    RpcModule,
};
use std::time::Duration;
use tokio::time::timeout;
use tracing::instrument;

#[rpc(server)]
pub trait Wifi {
    // async fn status(&self, ctx: CallContext) -> RpcResult<DabNetworkStatus>;
    #[method(name = "wifi.scan")]
    async fn scan(
        &self,
        ctx: CallContext,
        pair_request: Option<DabWifiScanRequest>,
    ) -> RpcResult<AccessPointList>;
    #[method(name = "wifi.connect")]
    async fn connect(
        &self,
        ctx: CallContext,
        pair_request: AccessPointRequest,
    ) -> RpcResult<AccessPoint>;
}

pub struct WifiImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
}

#[async_trait]
impl WifiServer for WifiImpl<RippleHelper> {
    #[instrument(skip(self))]
    async fn scan(
        &self,
        _ctx: CallContext,
        _wifi_request: Option<DabWifiScanRequest>,
    ) -> RpcResult<AccessPointList> {
        let scan_process = self
            .helper
            .send_dab(DabRequestPayload::Wifi(WifiRequest::Scan));

        let default_timeout = ConfigManager::get().get_default_scan_timeout().into();
        let scan_req = _wifi_request.unwrap_or(DabWifiScanRequest {
            timeout: Some(default_timeout),
        });
        let scan_timeout = scan_req.timeout.unwrap_or(default_timeout);
        match timeout(Duration::from_secs(scan_timeout), scan_process).await {
            Ok(resp) => match resp {
                Ok(dab_payload) => match dab_payload {
                    DabResponsePayload::WifiScanListResponse(value) => Ok(value),
                    _ => Err(jsonrpsee::core::Error::Custom(String::from(
                        "Wifi scan error response TBD",
                    ))),
                },
                Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Wifi scan error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Wifi scan timed out",
            ))),
        }
    }

    #[instrument(skip(self))]
    async fn connect(
        &self,
        _ctx: CallContext,
        connect_request: AccessPointRequest,
    ) -> RpcResult<AccessPoint> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Wifi(WifiRequest::Connect(
                connect_request,
            )))
            .await;
        match resp {
            Ok(dab_payload) => match dab_payload {
                DabResponsePayload::WifiConnectSuccessResponse(value) => Ok(value),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Wifi connect error response TBD",
                ))),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Wifi connect error response TBD",
            ))),
        }
    }
}

pub struct WifiRippleProvider;
pub struct WifiCapHandler;

impl IGetLoadedCaps for WifiCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::Short("protocol:wifi".into()),
            ])]),
        }
    }
}

impl RPCProvider<WifiImpl<RippleHelper>, WifiCapHandler> for WifiRippleProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        _platform_state: PlatformState,
    ) -> (RpcModule<WifiImpl<RippleHelper>>, WifiCapHandler) {
        let a = WifiImpl {
            helper: rhf.get(self.get_helper_variant()),
        };
        (a.into_rpc(), WifiCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![RippleHelperType::Dab]
    }
}
