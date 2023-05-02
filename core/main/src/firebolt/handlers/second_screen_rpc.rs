use std::collections::HashMap;

use crate::{
    firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState,
    utils::rpc_utils::rpc_add_event_listener,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::api::{
    firebolt::fb_general::{ListenRequest, ListenerResponse},
    gateway::rpc_gateway_api::CallContext,
};
use serde::{Deserialize, Serialize};

use super::device_rpc::{get_device_id, get_device_name};

pub const EVENT_SECOND_SCREEN_ON_LAUNCH_REQUEST: &'static str = "secondscreen.onLaunchRequest";
pub const EVENT_SECOND_SCREEN_ON_CLOSE_REQUEST: &'static str = "secondscreen.onCloseRequest";
pub const EVENT_SECOND_SCREEN_ON_FRIENDLY_NAME_CHANGED: &'static str =
    "secondscreen.onFriendlyNameChanged";

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

pub struct SecondScreenImpl {
    pub state: PlatformState,
}

#[async_trait]
impl SecondScreenServer for SecondScreenImpl {
    async fn device(&self, _ctx: CallContext, _param: SecondScreenDeviceInfo) -> RpcResult<String> {
        get_device_id(&self.state).await
    }

    async fn friendly_name(&self, _ctx: CallContext) -> RpcResult<String> {
        get_device_name(&self.state).await
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
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_SECOND_SCREEN_ON_LAUNCH_REQUEST,
        )
        .await
    }

    async fn on_close_request(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_SECOND_SCREEN_ON_CLOSE_REQUEST,
        )
        .await
    }

    async fn on_friendly_name_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(
            &self.state,
            ctx,
            request,
            EVENT_SECOND_SCREEN_ON_FRIENDLY_NAME_CHANGED,
        )
        .await
    }
}

pub struct SecondScreenRPCProvider;
impl RippleRPCProvider<SecondScreenImpl> for SecondScreenRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<SecondScreenImpl> {
        (SecondScreenImpl { state }).into_rpc()
    }
}
