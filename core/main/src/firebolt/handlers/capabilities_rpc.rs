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
use jsonrpsee::{core::RpcResult, proc_macros::rpc, RpcModule};
use ripple_sdk::api::{
    firebolt::{
        fb_capabilities::{
            CapEvent, CapInfoRpcRequest, CapListenRPCRequest, CapRequestRpcRequest, CapabilityInfo,
            CapabilityRole, FireboltCap, FireboltPermission, RoleInfo,
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
    #[method(name = "capabilities.granted")]
    async fn granted(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool>;
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
            .check_supported(&vec![FireboltPermission {
                cap: FireboltCap::Full(cap.capability),
                role: cap.role.unwrap_or(CapabilityRole::Use),
            }])
            .is_ok())
    }

    async fn available(&self, _ctx: CallContext, cap: RoleInfo) -> RpcResult<bool> {
        Ok(self
            .state
            .cap_state
            .generic
            .check_available(&vec![FireboltPermission {
                cap: FireboltCap::Full(cap.capability),
                role: cap.role.unwrap_or(CapabilityRole::Use),
            }])
            .is_ok())
    }

    async fn permitted(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool> {
        if let Ok(v) = self
            .state
            .cap_state
            .permitted_state
            .check_cap_role(&ctx.app_id, cap.clone())
        {
            return Ok(v);
        } else if PermissionHandler::fetch_and_store(&self.state, &ctx.app_id)
            .await
            .is_ok()
        {
            //successful fetch retry
            if let Ok(v) = self
                .state
                .cap_state
                .permitted_state
                .check_cap_role(&ctx.app_id, cap)
            {
                return Ok(v);
            }
        }
        Ok(false)
    }

    async fn granted(&self, ctx: CallContext, cap: RoleInfo) -> RpcResult<bool> {
        if let Ok(response) = is_granted(self.state.clone(), ctx, cap).await {
            return Ok(response);
        }
        Ok(false)
    }

    async fn info(
        &self,
        ctx: CallContext,
        request: CapInfoRpcRequest,
    ) -> RpcResult<Vec<CapabilityInfo>> {
        // let mut caps = Vec::new();
        // for cap in request.capabilities {
        //     if let Some(firebolt_cap) = FireboltCap::parse(cap) {
        //         caps.push(firebolt_cap);
        //     }
        // }
        // let cap_set = CapabilitySet::get_from_role(caps, Some(CapabilityRole::Use));
        let cap_set = request
            .capabilities
            .iter()
            .map(|cap| FireboltCap::Full(cap.to_owned()))
            .collect();
        if let Ok(a) = CapState::get_cap_info(&self.state, ctx, &cap_set).await {
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
        let req_list: Vec<FireboltPermission> = grants.clone().into();
        let permitted_result =
            PermissionHandler::check_permitted(&self.state, &ctx.app_id, &req_list).await;
        if permitted_result.is_ok() {
            let _ = GrantState::check_with_roles(
                &self.state,
                &ctx.clone().into(),
                &ctx.clone().into(),
                &req_list,
                false,
            )
            .await;
        }
        let request = grants
            .grants
            .iter()
            .map(|role_info| FireboltCap::Full(role_info.capability.to_owned()))
            .collect();

        if let Ok(a) = CapState::get_cap_info(&self.state, ctx, &request).await {
            Ok(a)
        } else {
            Err(jsonrpsee::core::Error::Custom(String::from(
                "Error retreiving Capability Info TBD",
            )))
        }
        // if let Ok(mut result) = CapState::get_cap_info(&self.state, ctx.clone(), request).await {
        //     // filter out Ungranted
        //     let intersect_firebolt_perm: Vec<FireboltPermission> = req_list
        //         .iter()
        //         .filter(|perm| {
        //             for cap_info in &result {
        //                 if &cap_info.capability == perm.cap.as_str().as_str() {
        //                     match perm.role {
        //                         CapabilityRole::Use => {
        //                             return cap_info._use.permitted
        //                                 && cap_info._use.granted.is_none()
        //                         }
        //                         CapabilityRole::Manage => {
        //                             return cap_info.manage.permitted
        //                                 && cap_info.manage.granted.is_none()
        //                         }
        //                         CapabilityRole::Provide => {
        //                             return cap_info.provide.permitted
        //                                 && cap_info.provide.granted.is_none()
        //                         }
        //                     }
        //                 }
        //             }
        //             return false;
        //         })
        //         .collect();
        //     let result = GrantState::check_with_roles(
        //         &self.state,
        //         &ctx.clone().into(),
        //         &ctx.clone().into(),
        //         &intersect_firebolt_perm,
        //         false,
        //     )
        //     .await;
        //     // let caps: Vec<String> = result
        //     //     .clone()
        //     //     .into_iter()
        //     //     .filter(|x| {
        //     //         x.details.is_some()
        //     //             && x.details.clone().unwrap().contains(&DenyReason::Ungranted)
        //     //     })
        //     //     .map(|x| x.capability)
        //     //     .collect();
        //     // let grants: Vec<RoleInfo> = grants
        //     //     .grants
        //     //     .into_iter()
        //     //     .filter(|x| caps.contains(&x.capability))
        //     //     .collect();
        //     // if grants.is_empty() {
        //     //     return Ok(result);
        //     // }

        //     // let mut cap_role = HashMap::new();
        //     // for role_info in &grants {
        //     //     cap_role.insert(
        //     //         role_info.capability.clone(),
        //     //         role_info.role.unwrap_or(CapabilityRole::Use),
        //     //     );
        //     // }

        //     // let ungranted_set: Vec<FireboltPermission> =
        //     //     grants.into_iter().map(|entry| entry.into()).collect();
        //     // let mut grant_denied_caps: Vec<String> = Vec::new();
        //     // if let Err(e) = GrantState::check_with_roles(
        //     //     &self.state,
        //     //     &ctx.clone().into(),
        //     //     &ctx.clone().into(),
        //     //     &ungranted_set,
        //     //     false,
        //     // )
        //     // .await
        //     // {
        //     //     for cap in e.caps {
        //     //         grant_denied_caps.push(cap.as_str());
        //     //     }
        //     // }

        //     // for info in result.iter_mut() {
        //     //     let capability = info.capability.clone();
        //     //     if let Some(role) = cap_role.get(&capability) {
        //     //         info.update_ungranted(role, grant_denied_caps.contains(&capability));
        //     //     }
        //     // }

        //     return Ok(result);
        // }

        // Err(jsonrpsee::core::Error::Custom(String::from(
        //     "Error retreiving Capability Info TBD",
        // )))
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
    ctx: CallContext,
    cap: RoleInfo,
) -> RpcResult<bool> {
    if state.open_rpc_state.is_app_excluded(&ctx.app_id) {
        return Ok(true);
    }
    if let Ok(v) = state
        .cap_state
        .permitted_state
        .check_cap_role(&ctx.app_id, cap.clone())
    {
        return Ok(v);
    } else if PermissionHandler::fetch_and_store(&state, &ctx.app_id)
        .await
        .is_ok()
    {
        //successful fetch retry
        if let Ok(v) = state
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
        .cap_state
        .grant_state
        .check_granted(&ctx.app_id, cap)
        .is_ok())
}
