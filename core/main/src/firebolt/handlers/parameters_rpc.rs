use jsonrpsee::{proc_macros::rpc, core::RpcResult};
use ripple_sdk::{api::{gateway::rpc_gateway_api::CallContext, firebolt::fb_parameters::AppInitParameters, apps::{AppResponse, AppRequest, AppMethod, AppManagerResponse}}, tokio::sync::oneshot};
use ripple_sdk::async_trait::async_trait;
use crate::{state::platform_state::PlatformState, utils::rpc_utils::rpc_await_oneshot};

#[rpc(server)]
pub trait Parameters {
    #[method(name = "parameters.initialization")]
    async fn initialization(&self, _ctx: CallContext) -> RpcResult<AppInitParameters>;
}

pub struct ParametersImpl {
    platform_state: PlatformState
}

#[async_trait]
impl ParametersServer for ParametersImpl {
    async fn initialization(&self, ctx: CallContext) -> RpcResult<AppInitParameters> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();
        let app_request = AppRequest::new(AppMethod::GetLaunchRequest(ctx.app_id), app_resp_tx);
        // TODO: Add privacy changes

        if let Ok(_) = self.platform_state.get_client().send_app_request(app_request).await {
            if let Ok(AppManagerResponse::LaunchRequest(r)) = rpc_await_oneshot(app_resp_rx).await {
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
        }

        Err(jsonrpsee::core::Error::Custom(String::from(
            "Internal Error",
        )))
    }
}