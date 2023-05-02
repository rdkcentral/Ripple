use std::collections::HashMap;

use jsonrpsee::core::async_trait;
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::RpcModule;
use tokio::sync::oneshot;
use tracing::instrument;

use crate::api::rpc::rpc_gateway::RPCProvider;
use crate::apps::app_events::AppEvents;
use crate::apps::app_events::ListenRequest;
use crate::apps::app_events::ListenerResponse;
use crate::apps::app_mgr::{AppError, AppManagerResponse, AppMethod, AppRequest};
use crate::helpers::error_util::rpc_await_oneshot;
use crate::helpers::ripple_helper::IRippleHelper;
use crate::helpers::ripple_helper::RippleHelperFactory;
use crate::helpers::ripple_helper::RippleHelperType;
use crate::managers::capability_manager::CapClassifiedRequest;
use crate::managers::capability_manager::FireboltCap;
use crate::managers::capability_manager::IGetLoadedCaps;
use crate::managers::capability_manager::RippleHandlerCaps;
use crate::platform_state::PlatformState;
use crate::{api::rpc::rpc_gateway::CallContext, helpers::ripple_helper::RippleHelper};
use serde::{Deserialize, Serialize};

use super::device::get_device_id;
use super::device::get_device_name;
use super::discovery::LaunchRequest;

pub const EVENT_SECOND_SCREEN_ON_LAUNCH_REQUEST: &'static str = "secondscreen.onLaunchRequest";
pub const EVENT_SECOND_SCREEN_ON_CLOSE_REQUEST: &'static str = "secondscreen.onCloseRequest";

#[derive(Serialize, Deserialize, Debug)]
pub struct SecondScreenDeviceInfo {
    #[serde(rename = "type")]
    pub _type: Option<String>,
}

#[rpc(server)]
pub trait SecondScreen {
    #[method(name = "secondscreen.device")]
    async fn device(&self, ctx: CallContext, param: SecondScreenDeviceInfo) -> RpcResult<String>;
    #[method(name = "secondscreen.friendlyName")]
    async fn friendly_name(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "secondscreen.protocols")]
    async fn protocols(&self, _ctx: CallContext) -> RpcResult<HashMap<String, bool>>;
    #[method(name = "secondscreen.onLaunchRequest")]
    async fn on_launch_request(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "secondscreen.onCloseRequest")]
    async fn on_close_request(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "secondscreen.onFriendlyNameChanged")]
    async fn on_friendly_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
}

pub struct SecondScreenImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}

impl SecondScreenImpl<RippleHelper> {
    async fn listen(
        &self,
        ctx: CallContext,
        request: ListenRequest,
        event_name: &'static str,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            event_name.to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: event_name,
        })
    }
}

#[async_trait]
impl SecondScreenServer for SecondScreenImpl<RippleHelper> {
    async fn device(&self, ctx: CallContext, param: SecondScreenDeviceInfo) -> RpcResult<String> {
        get_device_id(&self.helper, ctx).await
    }

    async fn friendly_name(&self, _ctx: CallContext) -> RpcResult<String> {
        get_device_name(&self.platform_state).await
    }

    async fn protocols(&self, _ctx: CallContext) -> RpcResult<HashMap<String, bool>> {
        let mut protocols = HashMap::<String, bool>::new();
        // TODO: This is what we do for XClass, but should this really be defined in the device manifest or queried via DAB?
        protocols.insert("dial2".into(), true);
        Ok(protocols)
    }

    async fn on_launch_request(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, EVENT_SECOND_SCREEN_ON_LAUNCH_REQUEST)
            .await
    }

    async fn on_close_request(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.listen(ctx, request, EVENT_SECOND_SCREEN_ON_CLOSE_REQUEST)
            .await
    }

    #[instrument(skip(self))]
    async fn on_friendly_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;

        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            "secondscreen.onFriendlyNameChanged".to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: "secondscreen.onFriendlyNameChanged",
        })
    }

}

pub struct SecondScreenRippleProvider;

pub struct SecondScreenCapHandler;

impl IGetLoadedCaps for SecondScreenCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::Short("protocol:dial".into()),
            ])]),
        }
    }
}

impl RPCProvider<SecondScreenImpl<RippleHelper>, SecondScreenCapHandler>
    for SecondScreenRippleProvider
{
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (
        RpcModule<SecondScreenImpl<RippleHelper>>,
        SecondScreenCapHandler,
    ) {
        let ss = SecondScreenImpl {
            helper: rhf.clone().get(self.get_helper_variant()),
            platform_state,
        };
        (ss.into_rpc(), SecondScreenCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![
            RippleHelperType::AppManager,
            RippleHelperType::Dab,
            RippleHelperType::Dpab,
            RippleHelperType::Cap,
        ]
    }
}
