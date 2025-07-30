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

use crate::state::platform_state::PlatformState;
use jsonrpsee::core::RpcResult;
use ripple_sdk::api::gateway::rpc_gateway_api::{CallContext, JsonRpcApiError, RpcRequest};
use serde_json::Value;

use super::endpoint_broker::BrokerCallback;

pub struct BrokerUtils;

impl BrokerUtils {
    pub async fn process_for_app_main_request(
        state: PlatformState,
        method: &str,
        params: Option<Value>,
        app_id: &str,
    ) -> RpcResult<Value> {
        let mut rpc_request = RpcRequest::internal(method, None).with_params(params);
        rpc_request.ctx.app_id = app_id.to_owned();
        Self::internal_request(state.clone(), rpc_request).await
    }

    pub async fn process_internal_main_request(
        state: PlatformState,
        method: &str,
        params: Option<Value>,
    ) -> RpcResult<Value> {
        Self::process_internal_request(state.clone(), None, method, params).await
    }

    pub async fn process_internal_request(
        state: PlatformState,
        on_behalf_of: Option<CallContext>,
        method: &str,
        params: Option<Value>,
    ) -> RpcResult<Value> {
        let rpc_request = RpcRequest::internal(method, on_behalf_of).with_params(params);
        state
            .add_api_stats(&rpc_request.ctx.request_id, method)
            .await;
        Self::internal_request(state.clone(), rpc_request).await
    }

    async fn internal_request(state: PlatformState, rpc_request: RpcRequest) -> RpcResult<Value> {
        let method = rpc_request.method.clone();
        match state.internal_rpc_request(&rpc_request).await {
            Ok(res) => match res.as_value() {
                Some(v) => Ok(v),
                None => Err(JsonRpcApiError::default()
                    .with_code(-32100)
                    .with_message(format!("failed to get {} : {:?}", method, res))
                    .into()),
            },
            Err(e) => Err(JsonRpcApiError::default()
                .with_code(-32100)
                .with_message(format!("failed to get {} : {}", method, e))
                .into()),
        }
    }

    pub async fn process_internal_subscription(
        state: PlatformState,
        method: &str,
        params: Option<Value>,
        app_id: Option<String>,
        callback: Option<BrokerCallback>,
    ) -> bool {
        let mut rpc_request = RpcRequest::internal(method, None).with_params(params);
        if let Some(app_id) = app_id {
            rpc_request.ctx.app_id = app_id.to_owned();
        }
        let state_c = state.clone();

        state
            .endpoint_state(|es| async move {
                es.handle_brokerage(
                    state_c,
                    rpc_request,
                    None,
                    callback,
                    Vec::new(),
                    None,
                    Vec::new(),
                )
                .await
            })
            .await
    }
}
