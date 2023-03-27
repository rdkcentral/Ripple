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
use crate::state::{cap::cap_state::CapState, platform_state::PlatformState};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ripple_sdk::api::{
    firebolt::{
        fb_capabilities::{
            CapEvent, CapInfoRpcRequest, CapListenRPCRequest, CapabilityInfo, FireboltCap, RoleInfo,
        },
        fb_general::ListenerResponse,
    },
    gateway::rpc_gateway_api::CallContext,
};
use ripple_sdk::async_trait::async_trait;

#[rpc(server)]
pub trait Capability {
    #[method(name = "capabilities.supported")]
    async fn supported(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool>;
    #[method(name = "capabilities.available")]
    async fn available(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool>;
    #[method(name = "capabilities.permitted")]
    async fn permitted(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool>;
    #[method(name = "capabilities.info")]
    async fn info(
        &self,
        ctx: CallContext,
        capabilities: CapInfoRpcRequest,
    ) -> RpcResult<Vec<CapabilityInfo>>;
    #[method(name = "capabilities.onAvailable")]
    async fn on_available(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "capabilities.onUnavailable")]
    async fn on_unavailable(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "capabilities.onGranted")]
    async fn on_granted(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "capabilities.onRevoked")]
    async fn on_revoked(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse>;
}

pub struct CapabilityImpl {
    state: PlatformState,
}

impl CapabilityImpl {
    pub async fn on_request_cap_event(
        &self,
        ctx: CallContext,
        request: CapListenRPCRequest,
        event: CapEvent,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        CapState::setup_listener(&self.state.clone(), ctx, event.clone(), request).await;
        Ok(ListenerResponse {
            listening: listen,
            event: format!("capabilities.{}", event.as_str()),
        })
    }
}

#[async_trait]
impl CapabilityServer for CapabilityImpl {
    async fn supported(&self, _ctx: CallContext, cap: RoleInfo) -> RpcResult<bool> {
        Ok(self
            .state
            .cap_state
            .generic
            .check_supported(&vec![FireboltCap::Full(cap.capability)])
            .is_ok())
    }

    async fn available(&self, _ctx: CallContext, cap: RoleInfo) -> RpcResult<bool> {
        Ok(self
            .state
            .cap_state
            .generic
            .check_available(&vec![FireboltCap::Full(cap.capability)])
            .is_ok())
    }

    async fn permitted(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool> {
        Ok(self
            .state
            .cap_state
            .permitted_state
            .check_cap_role(&ctx.app_id, cap))
    }

    async fn info(
        &self,
        ctx: CallContext,
        request: CapInfoRpcRequest,
    ) -> RpcResult<Vec<CapabilityInfo>> {
        if let Ok(a) = CapState::get_cap_info(&self.state, ctx, request.capabilities).await {
            Ok(a)
        } else {
            Err(jsonrpsee::core::Error::Custom(String::from(
                "Error retreiving Capability Info TBD",
            )))
        }
    }

    async fn on_available(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_cap_event(ctx, cap, CapEvent::OnAvailable)
            .await
    }

    async fn on_unavailable(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_cap_event(ctx, cap, CapEvent::OnUnavailable)
            .await
    }

    async fn on_granted(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_cap_event(ctx, cap, CapEvent::OnGranted)
            .await
    }

    async fn on_revoked(
        &self,
        ctx: CallContext,
        cap: CapListenRPCRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_cap_event(ctx, cap, CapEvent::OnRevoked)
            .await
    }
}
