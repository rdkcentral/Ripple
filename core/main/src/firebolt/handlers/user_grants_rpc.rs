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

use jsonrpsee::{
    core::{Error, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::{
    api::{
        apps::{AppManagerResponse, AppMethod, AppRequest, AppResponse},
        device::device_user_grants_data::{GrantEntry, GrantStateModify},
        firebolt::{
            fb_capabilities::{DenyReason, FireboltPermission, CAPABILITY_NOT_PERMITTED},
            fb_user_grants::{
                AppInfo, GetUserGrantsByAppRequest, GetUserGrantsByCapabilityRequest, GrantInfo,
                GrantRequest, UserGrantRequestParam,
            },
        },
        gateway::rpc_gateway_api::{AppIdentification, CallContext},
    },
    chrono::{DateTime, Utc},
    log::debug,
    tokio::sync::oneshot,
    utils::rpc_utils::rpc_error_with_code_result,
};

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::user_grants::GrantState,
    state::platform_state::PlatformState,
    utils::rpc_utils::{rpc_await_oneshot, rpc_err},
};
use ripple_sdk::async_trait::async_trait;
use std::{
    collections::HashSet,
    time::{Duration, SystemTime},
};

#[rpc(server)]
pub trait UserGrants {
    #[method(name = "usergrants.app")]
    async fn usergrants_app(
        &self,
        ctx: CallContext,
        request: GetUserGrantsByAppRequest,
    ) -> RpcResult<Vec<GrantInfo>>;
    #[method(name = "usergrants.device")]
    async fn usergrants_device(&self, ctx: CallContext) -> RpcResult<Vec<GrantInfo>>;
    #[method(name = "usergrants.capability")]
    async fn usergrants_capability(
        &self,
        ctx: CallContext,
        request: GetUserGrantsByCapabilityRequest,
    ) -> RpcResult<Vec<GrantInfo>>;
    #[method(name = "usergrants.grant")]
    async fn usergrants_grant(&self, ctx: CallContext, request: GrantRequest) -> RpcResult<()>;
    #[method(name = "usergrants.deny")]
    async fn usergrants_deny(&self, ctx: CallContext, request: GrantRequest) -> RpcResult<()>;
    #[method(name = "usergrants.clear")]
    async fn usergrants_clear(&self, ctx: CallContext, request: GrantRequest) -> RpcResult<()>;
    #[method(name = "usergrants.request")]
    async fn usergrants_request(
        &self,
        ctx: CallContext,
        request: UserGrantRequestParam,
    ) -> RpcResult<Vec<GrantInfo>>;
}

#[derive(Debug)]
pub struct UserGrantsImpl {
    pub platform_state: PlatformState,
}
impl UserGrantsImpl {
    async fn get_app_title(&self, app_id: &str) -> RpcResult<Option<String>> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request = AppRequest::new(AppMethod::GetAppName(app_id.into()), app_resp_tx);

        if self
            .platform_state
            .get_client()
            .send_app_request(app_request)
            .is_err()
        {
            return Err(rpc_err(format!(
                "Failed to get App Name for {}",
                app_id.to_owned()
            )));
        }

        let resp = rpc_await_oneshot(app_resp_rx).await?;

        if let AppManagerResponse::AppName(app_title) = resp? {
            return Ok(app_title);
        }

        Err(rpc_err(format!(
            "Failed to get App Title for {}",
            app_id.to_owned()
        )))
    }

    async fn create_grantinfo_from_grant_entry_list(
        &self,
        app_id: Option<String>,
        grant_entries: &HashSet<GrantEntry>,
    ) -> Vec<GrantInfo> {
        let app_name = match app_id.clone() {
            Some(id) => self.get_app_title(&id).await.ok().flatten(),
            None => None,
        };
        grant_entries
            .iter()
            .filter(|x| x.status.is_some() && x.lifespan.is_some())
            .map(move |x| UserGrantsImpl::transform(app_id.clone(), app_name.clone(), x))
            .collect()
    }

    //Transform GrantEntry to GrantInfo.  app_id None is for device.
    fn transform(
        app_id: Option<String>,
        app_name: Option<String>,
        entry: &GrantEntry,
    ) -> GrantInfo {
        GrantInfo {
            app: app_id.map(|x| AppInfo {
                id: x,
                title: app_name,
            }),
            state: entry
                .status
                .as_ref()
                .map_or("INVALID".to_owned(), |s| s.as_string().to_owned()),
            capability: entry.capability.to_owned(),
            role: entry.role.as_string().to_owned(),
            lifespan: entry.lifespan.as_ref().unwrap().as_string().to_owned(),
            expires: {
                entry.lifespan_ttl_in_secs.map(|ttl_secs| {
                    let expiry_system_time: SystemTime = SystemTime::UNIX_EPOCH
                        + entry.last_modified_time
                        + Duration::from_secs(ttl_secs);
                    let expiry_date_time: DateTime<Utc> = DateTime::from(expiry_system_time);
                    expiry_date_time.to_rfc3339()
                })
            },
        }
    }
}

#[async_trait]
impl UserGrantsServer for UserGrantsImpl {
    async fn usergrants_app(
        &self,
        _ctx: CallContext,
        request: GetUserGrantsByAppRequest,
    ) -> RpcResult<Vec<GrantInfo>> {
        let grant_entries = self
            .platform_state
            .cap_state
            .grant_state
            .get_grant_entries_for_app_id(request.app_id.clone());

        Ok(self
            .create_grantinfo_from_grant_entry_list(Some(request.app_id), &grant_entries)
            .await)
    }

    async fn usergrants_device(&self, _ctx: CallContext) -> RpcResult<Vec<GrantInfo>> {
        let grant_entries = self
            .platform_state
            .cap_state
            .grant_state
            .get_device_entries();

        Ok(self
            .create_grantinfo_from_grant_entry_list(None, &grant_entries)
            .await)
    }

    async fn usergrants_capability(
        &self,
        _ctx: CallContext,
        request: GetUserGrantsByCapabilityRequest,
    ) -> RpcResult<Vec<GrantInfo>> {
        let grant_enrtry_map = self
            .platform_state
            .cap_state
            .grant_state
            .get_grant_entries_for_capability(&request.capability);

        let mut combined_grant_entries: Vec<GrantInfo> = Vec::new();
        for (app_id, app_entries) in grant_enrtry_map.iter() {
            combined_grant_entries.extend(
                self.create_grantinfo_from_grant_entry_list(Some(app_id.clone()), app_entries)
                    .await,
            );
        }
        Ok(combined_grant_entries)
    }

    async fn usergrants_grant(&self, ctx: CallContext, request: GrantRequest) -> RpcResult<()> {
        let result = GrantState::update_grant(
            self.platform_state.clone(),
            GrantStateModify::Grant,
            &request.options.and_then(|x| x.app_id),
            request.role,
            request.capability,
            ctx,
        )
        .await;
        result.map_err(rpc_err)
    }

    async fn usergrants_deny(&self, ctx: CallContext, request: GrantRequest) -> RpcResult<()> {
        let result = GrantState::update_grant(
            self.platform_state.clone(),
            GrantStateModify::Deny,
            &request.options.and_then(|x| x.app_id),
            request.role,
            request.capability,
            ctx,
        )
        .await;
        result.map_err(rpc_err)
    }

    async fn usergrants_clear(&self, ctx: CallContext, request: GrantRequest) -> RpcResult<()> {
        let result = GrantState::update_grant(
            self.platform_state.clone(),
            GrantStateModify::Clear,
            &request.options.and_then(|x| x.app_id),
            request.role,
            request.capability,
            ctx,
        )
        .await;
        result.map_err(rpc_err)
    }
    async fn usergrants_request(
        &self,
        ctx: CallContext,
        request: UserGrantRequestParam,
    ) -> RpcResult<Vec<GrantInfo>> {
        let force = request
            .options
            .as_ref()
            .map(|x| x.force)
            .unwrap_or_default();

        let fb_perms: Vec<FireboltPermission> = request.clone().into();
        self.platform_state
            .cap_state
            .generic
            .check_supported(&fb_perms)
            .map_err(|err| Error::Custom(format!("{:?} not supported", err.caps)))?;
        let grant_entries = GrantState::check_with_roles(
            self.platform_state.clone(),
            &ctx.clone().into(),
            &AppIdentification {
                app_id: request.app_id.clone(),
            },
            &fb_perms,
            false,
            true,
            force,
        )
        .await;
        debug!("Check with roles result: {:?}", grant_entries);
        if let Err(grant_entries_err) = grant_entries {
            if DenyReason::AppNotInActiveState == grant_entries_err.reason {
                return rpc_error_with_code_result::<Vec<GrantInfo>>(  "Capability cannot be used when app is not in foreground state due to requiring a user grant".to_owned(), CAPABILITY_NOT_PERMITTED);
            }
        }
        self.usergrants_app(
            ctx,
            GetUserGrantsByAppRequest {
                app_id: request.app_id,
            },
        )
        .await
    }
}

pub struct UserGrantsRPCProvider;

impl RippleRPCProvider<UserGrantsImpl> for UserGrantsRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<UserGrantsImpl> {
        (UserGrantsImpl {
            platform_state: state,
        })
        .into_rpc()
    }
}
