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

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::user_grants::GrantState,
    state::{
        cap::{cap_state::CapState, permitted_state::PermissionHandler},
        platform_state::PlatformState,
    },
};
use jsonrpsee::{
    core::{Error, RpcResult},
    proc_macros::rpc,
    RpcModule,
};

use ripple_sdk::async_trait::async_trait;
use ripple_sdk::{
    api::{
        firebolt::{
            fb_capabilities::{
                CapEvent, CapInfoRpcRequest, CapListenRPCRequest, CapRPCRequest,
                CapRequestRpcRequest, CapabilityInfo, DenyReason, FireboltPermission, RoleInfo,
            },
            fb_general::ListenerResponse,
        },
        gateway::rpc_gateway_api::CallContext,
    },
    utils::error::RippleError,
};

#[rpc(server)]
pub trait Capability {
    #[method(name = "ripple.isPermitted")]
    async fn get_capability_permit(&self, ctx: CallContext, role: RoleInfo) -> RpcResult<bool>;
    #[method(name = "capabilities.supported")]
    async fn supported(&self, ctx: CallContext, cap: CapRPCRequest) -> RpcResult<bool>;
    #[method(name = "capabilities.available")]
    async fn available(&self, ctx: CallContext, cap: CapRPCRequest) -> RpcResult<bool>;
    #[method(name = "capabilities.permitted")]
    async fn permitted(&self, ctx: CallContext, cap: CapRPCRequest) -> RpcResult<bool>;
    #[method(name = "capabilities.granted")]
    async fn granted(&self, ctx: CallContext, cap: CapRPCRequest) -> RpcResult<Option<bool>>;
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
    #[method(name = "capabilities.request")]
    async fn cap_set_request(
        &self,
        ctx: CallContext,
        grants: CapRequestRpcRequest,
    ) -> RpcResult<Vec<CapabilityInfo>>;
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
        CapState::setup_listener(self.state.clone(), ctx, event.clone(), request).await;
        Ok(ListenerResponse {
            listening: listen,
            event: format!("capabilities.{}", event.as_str()),
        })
    }
}

#[async_trait]
impl CapabilityServer for CapabilityImpl {
    async fn get_capability_permit(&self, ctx: CallContext, role: RoleInfo) -> RpcResult<bool> {
        is_permitted(self.state.clone(), &ctx, &role).await
    }

    async fn supported(&self, _ctx: CallContext, cap: CapRPCRequest) -> RpcResult<bool> {
        Ok(self
            .state
            .cap_state
            .generic
            .check_supported(&[FireboltPermission {
                cap: cap.capability,
                role: cap.options.unwrap_or_default().role,
            }])
            .is_ok())
    }

    async fn available(&self, _ctx: CallContext, cap: CapRPCRequest) -> RpcResult<bool> {
        Ok(self
            .state
            .cap_state
            .generic
            .check_available(&vec![FireboltPermission {
                cap: cap.capability,
                role: cap.options.unwrap_or_default().role,
            }])
            .is_ok())
    }

    async fn permitted(&self, ctx: CallContext, cap: CapRPCRequest) -> RpcResult<bool> {
        if self.state.open_rpc_state.is_app_excluded(&ctx.app_id) {
            return Ok(true);
        }

        if let Ok(v) = self
            .state
            .cap_state
            .permitted_state
            .check_cap_role(&ctx.app_id, &cap.clone().into())
        {
            return Ok(v);
        } else if PermissionHandler::fetch_and_store(self.state.clone(), &ctx.app_id, true)
            .await
            .is_ok()
        {
            //successful fetch retry
            if let Ok(v) = self
                .state
                .cap_state
                .permitted_state
                .check_cap_role(&ctx.app_id, &cap.into())
            {
                return Ok(v);
            }
        }
        Ok(false)
    }

    async fn granted(&self, ctx: CallContext, cap: CapRPCRequest) -> RpcResult<Option<bool>> {
        let role_info = cap.into();
        let granted_res = self.state.cap_state.grant_state.check_granted(
            self.state.clone(),
            &ctx.app_id,
            role_info,
        );
        match granted_res {
            Ok(grant) => Ok(Some(grant)),
            Err(RippleError::Permission(DenyReason::Ungranted)) => Ok(None),
            Err(RippleError::Permission(DenyReason::GrantDenied)) => Ok(Some(false)),
            Err(_) => Err(Error::Custom("Unable to get user grants".to_owned())),
        }
    }

    async fn info(
        &self,
        ctx: CallContext,
        request: CapInfoRpcRequest,
    ) -> RpcResult<Vec<CapabilityInfo>> {
        if request.capabilities.is_empty() {
            return Err(jsonrpsee::core::Error::Custom(String::from(
                "Error invalid input capabilities are empty",
            )));
        }
        let cap_set = request.capabilities;
        if let Ok(a) = CapState::get_cap_info(self.state.clone(), ctx, &cap_set).await {
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

    async fn cap_set_request(
        &self,
        ctx: CallContext,
        grants: CapRequestRpcRequest,
    ) -> RpcResult<Vec<CapabilityInfo>> {
        let mut fb_perms: Vec<FireboltPermission> = grants.clone().into();
        let mut cap_info = Vec::new();
        if let Err(e) = self
            .state
            .clone()
            .cap_state
            .generic
            .check_supported(&fb_perms)
        {
            fb_perms.retain(|x| !e.caps.contains(&x.cap));
            for cap in e.caps {
                cap_info.push(CapabilityInfo::get(
                    cap.as_str(),
                    Some(DenyReason::Unsupported),
                ))
            }
        }
        let permitted_result: Result<
            (),
            ripple_sdk::api::firebolt::fb_capabilities::DenyReasonWithCap,
        > = PermissionHandler::check_permitted(self.state.clone(), &ctx.app_id, &fb_perms).await;
        if permitted_result.is_ok() {
            let _ = GrantState::check_with_roles(
                self.state.clone(),
                &ctx.clone().into(),
                &ctx.clone().into(),
                &fb_perms,
                false,
                true,
                false,
            )
            .await;
        }
        let request = grants
            .grants
            .iter()
            .map(|role_info| role_info.capability.clone())
            .collect();

        if let Ok(a) = CapState::get_cap_info(self.state.clone(), ctx, &request).await {
            cap_info.extend(a);
            Ok(cap_info)
        } else {
            Err(jsonrpsee::core::Error::Custom(String::from(
                "Error retreiving Capability Info TBD",
            )))
        }
    }
}

pub struct CapRPCProvider;
impl RippleRPCProvider<CapabilityImpl> for CapRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<CapabilityImpl> {
        (CapabilityImpl { state }).into_rpc()
    }
}

pub async fn is_permitted(
    state: PlatformState,
    ctx: &CallContext,
    cap: &RoleInfo,
) -> RpcResult<bool> {
    if state.open_rpc_state.is_app_excluded(&ctx.app_id) {
        return Ok(true);
    }
    if let Ok(v) = (state)
        .cap_state
        .permitted_state
        .check_cap_role(&ctx.app_id, cap)
    {
        return Ok(v);
    } else if PermissionHandler::fetch_and_store(state.clone(), &ctx.app_id, true)
        .await
        .is_ok()
    {
        //successful fetch retry
        if let Ok(v) = state
            .clone()
            .cap_state
            .permitted_state
            .check_cap_role(&ctx.app_id, cap)
        {
            return Ok(v);
        }
    }
    Ok(false)
}

pub async fn is_granted(state: PlatformState, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool> {
    Ok(state
        .clone()
        .cap_state
        .clone()
        .grant_state
        .check_granted(state.clone(), &ctx.app_id, cap)
        .is_ok())
}
