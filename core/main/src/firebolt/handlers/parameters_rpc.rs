use crate::{
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    apps::app_mgr::{AppError, AppManagerResponse, AppMethod, AppRequest},
    helpers::{
        error_util::rpc_await_oneshot,
        ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
    },
    managers::capability_manager::{
        CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
    },
    platform_state::PlatformState,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use super::{discovery::NavigationIntent, privacy};

#[derive(Serialize, Debug, Clone)]
pub struct AppInitParameters {
    pub us_privacy: Option<String>,
    pub lmt: Option<u16>,
    pub discovery: Option<DiscoveryEvent>,
    #[serde(rename = "secondScreen", skip_serializing_if = "Option::is_none")]
    pub second_screen: Option<SecondScreenEvent>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SecondScreenEvent {
    #[serde(rename = "type")]
    pub _type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct DiscoveryEvent {
    #[serde(rename = "navigateTo")]
    pub navigate_to: NavigationIntent,
}

#[rpc(server)]
pub trait Parameters {
    #[method(name = "parameters.initialization")]
    async fn initialization(&self, _ctx: CallContext) -> RpcResult<AppInitParameters>;
}

pub struct ParametersImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    platform_state: PlatformState,
}

#[async_trait]
impl ParametersServer for ParametersImpl<RippleHelper> {
    async fn initialization(&self, ctx: CallContext) -> RpcResult<AppInitParameters> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();
        let app_request = AppRequest {
            method: AppMethod::GetLaunchRequest(ctx.app_id),
            resp_tx: Some(app_resp_tx),
        };
        let privacy_data =
            privacy::get_allow_app_content_ad_targeting_settings(&self.platform_state).await;

        let _ = self.helper.send_app_request(app_request).await?;
        let resp = rpc_await_oneshot(app_resp_rx).await?;
        if let AppManagerResponse::LaunchRequest(launch_req) = resp? {
            return Ok(AppInitParameters {
                us_privacy: privacy_data.get(privacy::US_PRIVACY_KEY).cloned(),
                lmt: privacy_data
                    .get(privacy::LMT_KEY)
                    .and_then(|x| x.parse::<u16>().ok()),
                discovery: Some(DiscoveryEvent {
                    navigate_to: launch_req.get_intent(),
                }),
                second_screen: None,
            });
        }

        Err(jsonrpsee::core::Error::Custom(String::from(
            "Internal Error",
        )))
    }
}

pub struct ParametersProvider;

pub struct ParametersCapHandler;

impl IGetLoadedCaps for ParametersCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps { caps: Some(vec![]) }
    }
}

impl RPCProvider<ParametersImpl<RippleHelper>, ParametersCapHandler> for ParametersProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (
        RpcModule<ParametersImpl<RippleHelper>>,
        ParametersCapHandler,
    ) {
        let a = ParametersImpl {
            helper: rhf.get(self.get_helper_variant()),
            platform_state,
        };
        (a.into_rpc(), ParametersCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![RippleHelperType::AppManager]
    }
}
