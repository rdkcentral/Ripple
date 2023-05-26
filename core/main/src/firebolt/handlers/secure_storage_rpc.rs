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

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    tracing::error,
    RpcModule,
};
use ripple_sdk::api::{
    firebolt::fb_secure_storage::{
        GetRequest, RemoveRequest, SecureStorageGetRequest, SecureStorageRemoveRequest,
        SecureStorageRequest, SecureStorageResponse, SecureStorageSetRequest, SetRequest,
    },
    gateway::rpc_gateway_api::CallContext,
};

use crate::{firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState};

#[rpc(server)]
pub trait SecureStorage {
    #[method(name = "securestorage.get")]
    async fn get(&self, ctx: CallContext, request: GetRequest) -> RpcResult<String>;
    #[method(name = "securestorage.set")]
    async fn set(&self, ctx: CallContext, request: SetRequest) -> RpcResult<()>;
    #[method(name = "securestorage.remove")]
    async fn remove(&self, ctx: CallContext, request: RemoveRequest) -> RpcResult<()>;
}
pub struct SecureStorageImpl {
    pub state: PlatformState,
}

#[async_trait]
impl SecureStorageServer for SecureStorageImpl {
    async fn get(&self, ctx: CallContext, request: GetRequest) -> RpcResult<String> {
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

    async fn set(&self, ctx: CallContext, request: SetRequest) -> RpcResult<()> {
        match self
            .state
            .get_client()
            .send_extn_request(SecureStorageRequest::Set(SecureStorageSetRequest {
                app_id: ctx.app_id,
                value: request.value,
                options: request.options,
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

    async fn remove(&self, ctx: CallContext, request: RemoveRequest) -> RpcResult<()> {
        match self
            .state
            .get_client()
            .send_extn_request(SecureStorageRequest::Remove(SecureStorageRemoveRequest {
                app_id: ctx.app_id,
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

pub struct SecureStorageRPCProvider;
impl RippleRPCProvider<SecureStorageImpl> for SecureStorageRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<SecureStorageImpl> {
        (SecureStorageImpl { state }).into_rpc()
    }
}
