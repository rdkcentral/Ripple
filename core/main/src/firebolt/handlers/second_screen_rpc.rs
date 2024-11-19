// Copyright 2023 Comcast Cable Communications Management, LLC
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
//
// SPDX-License-Identifier: Apache-2.0
//

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
    firebolt::{
        fb_general::{ListenRequest, ListenerResponse},
        fb_secondscreen::SECOND_SCREEN_EVENT_ON_LAUNCH_REQUEST,
    },
    gateway::rpc_gateway_api::CallContext,
};

use super::device_rpc::get_device_name;

pub const EVENT_SECOND_SCREEN_ON_CLOSE_REQUEST: &str = "secondscreen.onCloseRequest";
pub const EVENT_SECOND_SCREEN_ON_FRIENDLY_NAME_CHANGED: &str = "secondscreen.onFriendlyNameChanged";

#[rpc(server)]
pub trait SecondScreen {
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
    async fn friendly_name(&self, _ctx: CallContext) -> RpcResult<String> {
        get_device_name(&self.state).await
    }

    async fn protocols(&self, _ctx: CallContext) -> RpcResult<HashMap<String, bool>> {
        let mut protocols = HashMap::<String, bool>::new();
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
            SECOND_SCREEN_EVENT_ON_LAUNCH_REQUEST,
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
