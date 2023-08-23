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
    tracing::error,
    RpcModule,
};
use ripple_sdk::api::{
    firebolt::fb_secure_storage::{
        GetRequest, RemoveAppRequest, RemoveRequest, SecureStorageGetRequest,
        SecureStorageRemoveRequest, SecureStorageRequest, SecureStorageResponse,
        SecureStorageSetRequest, SetAppRequest, SetRequest, StorageSetOptions,
    },
    gateway::rpc_gateway_api::CallContext,
};

use crate::{firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState};

#[rpc(server)]
pub trait SecureStorage {
    #[method(name = "securestorage.get")]
    async fn get_rpc(&self, ctx: CallContext, request: GetRequest) -> RpcResult<String>;
    #[method(name = "securestorage.set")]
    async fn set_rpc(&self, ctx: CallContext, request: SetRequest) -> RpcResult<()>;
    #[method(name = "securestorage.remove")]
    async fn remove_rpc(&self, ctx: CallContext, request: RemoveRequest) -> RpcResult<()>;
    #[method(name = "securestorage.setForApp")]
    async fn set_for_app_rpc(&self, ctx: CallContext, request: SetAppRequest) -> RpcResult<()>;
    #[method(name = "securestorage.removeForApp")]
    async fn remove_for_app_rpc(
        &self,
        ctx: CallContext,
        request: RemoveAppRequest,
    ) -> RpcResult<()>;
}
pub struct SecureStorageImpl {
    pub state: PlatformState,
}

impl SecureStorageImpl {
    pub fn rpc(pst: &PlatformState) -> RpcModule<SecureStorageImpl> {
        SecureStorageImpl { state: pst.clone() }.into_rpc()
    }

    async fn set(&self, app_id: String, request: SetRequest) -> RpcResult<()> {
        match self
            .state
            .get_client()
            .send_extn_request(SecureStorageRequest::Set(SecureStorageSetRequest {
                app_id,
                value: request.value,
                options: request.options.map(|op| StorageSetOptions { ttl: op.ttl }),
                scope: request.scope,
                key: request.key,
                distributor_session: self.state.session_state.get_account_session().unwrap(),
            }))
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                error!("error={:?}", err);
                Err(jsonrpsee::core::Error::Custom(
                    "Error setting value".to_owned(),
                ))
            }
        }
    }

    async fn remove(&self, app_id: String, request: RemoveRequest) -> RpcResult<()> {
        match self
            .state
            .get_client()
            .send_extn_request(SecureStorageRequest::Remove(SecureStorageRemoveRequest {
                app_id,
                scope: request.scope,
                key: request.key,
                distributor_session: self.state.session_state.get_account_session().unwrap(),
            }))
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                error!("error={:?}", err);
                Err(jsonrpsee::core::Error::Custom(
                    "Error setting value".to_owned(),
                ))
            }
        }
    }
}

#[async_trait]
impl SecureStorageServer for SecureStorageImpl {
    async fn get_rpc(&self, ctx: CallContext, request: GetRequest) -> RpcResult<String> {
        match self
            .state
            .get_client()
            .send_extn_request(SecureStorageRequest::Get(SecureStorageGetRequest {
                app_id: ctx.app_id,
                scope: request.scope,
                key: request.key,
                distributor_session: self.state.session_state.get_account_session().unwrap(),
            }))
            .await
        {
            Ok(response) => match response.payload.extract().unwrap() {
                SecureStorageResponse::Get(value) => Ok(value.value.unwrap_or(String::from(""))),
                _ => Err(jsonrpsee::core::Error::Custom(String::from(
                    "Secure Storage Response error response TBD",
                ))),
            },
            Err(err) => {
                error!("error={:?}", err);
                Err(jsonrpsee::core::Error::Custom(
                    "Error getting value".to_owned(),
                ))
            }
        }
    }

    async fn set_rpc(&self, ctx: CallContext, request: SetRequest) -> RpcResult<()> {
        self.set(ctx.app_id, request).await
    }

    async fn remove_rpc(&self, ctx: CallContext, request: RemoveRequest) -> RpcResult<()> {
        self.remove(ctx.app_id, request).await
    }

    async fn set_for_app_rpc(&self, _ctx: CallContext, request: SetAppRequest) -> RpcResult<()> {
        self.set(
            request.app_id,
            SetRequest {
                key: request.key,
                value: request.value,
                options: request.options,
                scope: request.scope,
            },
        )
        .await
    }
    async fn remove_for_app_rpc(
        &self,
        _ctx: CallContext,
        request: RemoveAppRequest,
    ) -> RpcResult<()> {
        self.remove(
            request.app_id,
            RemoveRequest {
                key: request.key,
                scope: request.scope,
            },
        )
        .await
    }
}

pub struct SecureStorageRPCProvider;
impl RippleRPCProvider<SecureStorageImpl> for SecureStorageRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<SecureStorageImpl> {
        (SecureStorageImpl { state }).into_rpc()
    }
}
