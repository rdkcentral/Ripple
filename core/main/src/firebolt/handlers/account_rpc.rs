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

pub struct AccountRPCProvider;
impl RippleRPCProvider<AccountImpl> for AccountRPCProvider {
    fn provide(platform_state: PlatformState) -> RpcModule<AccountImpl> {
        (AccountImpl { platform_state }).into_rpc()
    }
}
