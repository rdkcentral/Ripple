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
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::{ApiProtocol, CallContext, RpcRequest},
        session::AccountSessionTokenRequest,
    },
    extn::extn_client_message::{ExtnMessage, ExtnResponse},
    utils::error::RippleError,
};
use serde_json::json;

use crate::{
    firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState,
    utils::rpc_utils::rpc_err,
};

const KEY_FIREBOLT_ACCOUNT_UID: &str = "fireboltAccountUid";

#[rpc(server)]
pub trait Account {
    #[method(name = "account.session")]
    async fn session(&self, ctx: CallContext, a_t_r: AccountSessionTokenRequest) -> RpcResult<()>;
    #[method(name = "account.id")]
    async fn id(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "account.uid")]
    async fn uid(&self, ctx: CallContext) -> RpcResult<String>;
}

#[derive(Debug, Clone)]
pub struct AccountImpl {
    pub platform_state: PlatformState,
}

#[async_trait]
impl AccountServer for AccountImpl {
    async fn session(
        &self,
        mut _ctx: CallContext,
        a_t_r: AccountSessionTokenRequest,
    ) -> RpcResult<()> {
        self.platform_state
            .session_state
            .insert_session_token(a_t_r.token.clone());
        _ctx.protocol = ApiProtocol::Extn;

        let mut platform_state = self.platform_state.clone();
        platform_state
            .metrics
            .add_api_stats(&_ctx.request_id, "account.setServiceAccessToken");

        let success = rpc_request_setter(
            self.platform_state
                .get_client()
                .get_extn_client()
                .main_internal_request(RpcRequest {
                    ctx: _ctx.clone(),
                    method: "account.setServiceAccessToken".into(),
                    params_json: RpcRequest::prepend_ctx(
                        Some(json!({"token": a_t_r.token, "expires": a_t_r.expires_in})),
                        &_ctx,
                    ),
                })
                .await,
        );
        if !success {
            return Err(rpc_err("session error response TBD"));
        }
        Ok(())
    }

    async fn id(&self, _ctx: CallContext) -> RpcResult<String> {
        self.id().await
    }

    async fn uid(&self, ctx: CallContext) -> RpcResult<String> {
        crate::utils::common::get_uid(&self.platform_state, ctx.app_id, KEY_FIREBOLT_ACCOUNT_UID)
            .await
    }
}
impl AccountImpl {
    pub async fn id(&self) -> RpcResult<String> {
        if let Some(session) = self.platform_state.session_state.get_account_session() {
            Ok(session.account_id)
        } else {
            Err(rpc_err("Account.id: some failure"))
        }
    }
}

fn rpc_request_setter(response: Result<ExtnMessage, RippleError>) -> bool {
    if response.clone().is_ok() {
        if let Ok(res) = response {
            if let Some(ExtnResponse::Value(v)) = res.payload.extract::<ExtnResponse>() {
                if v.is_boolean() {
                    if let Some(b) = v.as_bool() {
                        return b;
                    }
                }
            }
        }
    }
    false
}

pub struct AccountRPCProvider;
impl RippleRPCProvider<AccountImpl> for AccountRPCProvider {
    fn provide(platform_state: PlatformState) -> RpcModule<AccountImpl> {
        (AccountImpl { platform_state }).into_rpc()
    }
}
