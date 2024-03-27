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

use crate::utils::rpc_utils::rpc_err;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    tracing::error,
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::fb_secure_storage::{
            SecureStorageClearRequest, SecureStorageGetRequest, SecureStorageRemoveRequest,
            SecureStorageRequest, SecureStorageResponse, SecureStorageSetRequest,
        },
        gateway::rpc_gateway_api::CallContext,
    },
    extn::extn_client_message::ExtnResponse,
    utils::error::RippleError,
};

use crate::{firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState};

macro_rules! return_if_app_id_missing {
    ($app_id:expr) => {
        if ($app_id.is_none()) {
            let msg = "missing field `app_id`";
            error!(msg);
            return Err(rpc_err(msg));
        }
    };
}

fn get_err_msg(err: RippleError) -> jsonrpsee::core::Error {
    jsonrpsee::core::Error::Custom(format!(
        "Error getting value {}",
        match err {
            RippleError::ExtnError => "(Server Error)",
            RippleError::ApiAuthenticationFailed => "(Session Missing)",
            _ => "",
        }
        .to_owned()
    ))
}

#[rpc(server)]
pub trait SecureStorage {
    #[method(name = "securestorage.get")]
    async fn get_rpc(
        &self,
        ctx: CallContext,
        request: SecureStorageGetRequest,
    ) -> RpcResult<Option<String>>;
    #[method(name = "securestorage.set")]
    async fn set_rpc(&self, ctx: CallContext, request: SecureStorageSetRequest) -> RpcResult<()>;
    #[method(name = "securestorage.remove")]
    async fn remove_rpc(
        &self,
        ctx: CallContext,
        request: SecureStorageRemoveRequest,
    ) -> RpcResult<()>;
    #[method(name = "securestorage.clear")]
    async fn clear_rpc(
        &self,
        ctx: CallContext,
        request: SecureStorageClearRequest,
    ) -> RpcResult<()>;
    #[method(name = "securestorage.setForApp")]
    async fn set_for_app_rpc(
        &self,
        ctx: CallContext,
        request: SecureStorageSetRequest,
    ) -> RpcResult<()>;
    #[method(name = "securestorage.removeForApp")]
    async fn remove_for_app_rpc(
        &self,
        ctx: CallContext,
        request: SecureStorageRemoveRequest,
    ) -> RpcResult<()>;
    #[method(name = "securestorage.clearForApp")]
    async fn clear_for_app_rpc(
        &self,
        ctx: CallContext,
        request: SecureStorageClearRequest,
    ) -> RpcResult<()>;
}
pub struct SecureStorageImpl {
    pub state: PlatformState,
}

impl SecureStorageImpl {
    pub fn rpc(pst: &PlatformState) -> RpcModule<SecureStorageImpl> {
        SecureStorageImpl { state: pst.clone() }.into_rpc()
    }

    async fn set(&self, request: SecureStorageSetRequest) -> RpcResult<()> {
        if let Some(session) = self.state.session_state.get_account_session() {
            match self
                .state
                .get_client()
                .send_extn_request(SecureStorageRequest::Set(request, session))
                .await
            {
                Ok(response) => {
                    if let Some(ExtnResponse::SecureStorage(_r)) = response.extract() {
                        Ok(())
                    } else {
                        Err(get_err_msg(RippleError::ExtnError))
                    }
                }
                Err(e) => Err(get_err_msg(e)),
            }
        } else {
            Err(get_err_msg(RippleError::ApiAuthenticationFailed))
        }
    }

    async fn remove(&self, request: SecureStorageRemoveRequest) -> RpcResult<()> {
        if let Some(session) = self.state.session_state.get_account_session() {
            match self
                .state
                .get_client()
                .send_extn_request(SecureStorageRequest::Remove(request, session))
                .await
            {
                Ok(response) => {
                    if let Some(ExtnResponse::SecureStorage(_r)) = response.extract() {
                        Ok(())
                    } else {
                        Err(get_err_msg(RippleError::ExtnError))
                    }
                }
                Err(e) => Err(get_err_msg(e)),
            }
        } else {
            Err(get_err_msg(RippleError::ApiAuthenticationFailed))
        }
    }
    async fn clear(&self, request: SecureStorageClearRequest) -> RpcResult<()> {
        if let Some(session) = self.state.session_state.get_account_session() {
            match self
                .state
                .get_client()
                .send_extn_request(SecureStorageRequest::Clear(request, session))
                .await
            {
                Ok(response) => {
                    if let Some(ExtnResponse::SecureStorage(_r)) = response.extract() {
                        Ok(())
                    } else {
                        Err(get_err_msg(RippleError::ExtnError))
                    }
                }
                Err(e) => Err(get_err_msg(e)),
            }
        } else {
            Err(get_err_msg(RippleError::ApiAuthenticationFailed))
        }
    }
}

#[async_trait]
impl SecureStorageServer for SecureStorageImpl {
    async fn get_rpc(
        &self,
        ctx: CallContext,
        request: SecureStorageGetRequest,
    ) -> RpcResult<Option<String>> {
        if let Some(session) = self.state.session_state.get_account_session() {
            match self
                .state
                .get_client()
                .send_extn_request(SecureStorageRequest::Get(ctx.app_id, request, session))
                .await
            {
                Ok(response) => {
                    if let Some(ExtnResponse::SecureStorage(SecureStorageResponse::Get(v))) =
                        response.extract()
                    {
                        Ok(v.value)
                    } else {
                        Err(get_err_msg(RippleError::ExtnError))
                    }
                }
                Err(e) => Err(get_err_msg(e)),
            }
        } else {
            Err(get_err_msg(RippleError::ApiAuthenticationFailed))
        }
    }

    async fn set_rpc(
        &self,
        ctx: CallContext,
        mut request: SecureStorageSetRequest,
    ) -> RpcResult<()> {
        request.app_id = Some(ctx.app_id);
        self.set(request).await
    }

    async fn remove_rpc(
        &self,
        ctx: CallContext,
        mut request: SecureStorageRemoveRequest,
    ) -> RpcResult<()> {
        request.app_id = Some(ctx.app_id);
        self.remove(request).await
    }

    async fn clear_rpc(
        &self,
        ctx: CallContext,
        mut request: SecureStorageClearRequest,
    ) -> RpcResult<()> {
        request.app_id = Some(ctx.app_id);
        self.clear(request).await
    }

    async fn set_for_app_rpc(
        &self,
        _ctx: CallContext,
        request: SecureStorageSetRequest,
    ) -> RpcResult<()> {
        return_if_app_id_missing!(request.app_id);
        self.set(request).await
    }
    async fn remove_for_app_rpc(
        &self,
        _ctx: CallContext,
        request: SecureStorageRemoveRequest,
    ) -> RpcResult<()> {
        return_if_app_id_missing!(request.app_id);
        self.remove(request).await
    }
    async fn clear_for_app_rpc(
        &self,
        _ctx: CallContext,
        request: SecureStorageClearRequest,
    ) -> RpcResult<()> {
        return_if_app_id_missing!(request.app_id);
        self.clear(request).await
    }
}

pub struct SecureStorageRPCProvider;
impl RippleRPCProvider<SecureStorageImpl> for SecureStorageRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<SecureStorageImpl> {
        (SecureStorageImpl { state }).into_rpc()
    }
}
