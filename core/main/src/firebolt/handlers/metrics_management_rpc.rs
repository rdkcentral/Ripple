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
use jsonrpsee::{core::RpcResult, proc_macros::rpc, RpcModule};
use ripple_sdk::{api::gateway::rpc_gateway_api::CallContext, async_trait::async_trait};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState};

#[derive(Deserialize, Debug, Clone)]
pub struct MetricsContextParams {
    context: MetricsContext,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetricsContext {
    pub device_session_id: Option<String>,
}

impl MetricsContext {
    fn is_valid_key(&self, key: &str) -> bool {
        let metrics_context_fields = match serde_json::to_value(self) {
            Ok(val) => {
                let field_map = val.as_object().unwrap();
                field_map.keys().map(|x| x.to_string()).collect::<Vec<_>>()
            }
            Err(_) => return false,
        };
        metrics_context_fields.contains(&key.to_string())
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetricsContextKeys {
    #[serde(deserialize_with = "validate_metrics_context_keys")]
    pub keys: Vec<String>,
}

fn validate_metrics_context_keys<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let keys: Vec<String> = Vec::deserialize(deserializer)?;
    let metics_context = MetricsContext::default();
    for key in &keys {
        if !metics_context.is_valid_key(key.as_str()) {
            return Err(serde::de::Error::custom(format!("Invalid key : {}", key)));
        }
    }
    Ok(keys)
}

#[rpc(server)]
pub trait MetricsManagement {
    #[method(name = "metricsmanagement.addContext")]
    async fn add_context(
        &self,
        ctx: CallContext,
        context_params: MetricsContextParams,
    ) -> RpcResult<()>;
    #[method(name = "metricsmanagement.removeContext")]
    async fn remove_context(&self, ctx: CallContext, request: MetricsContextKeys) -> RpcResult<()>;
}

pub struct MetricsManagementImpl {
    pub state: PlatformState,
}

#[async_trait]
impl MetricsManagementServer for MetricsManagementImpl {
    async fn add_context(
        &self,
        _ctx: CallContext,
        context_params: MetricsContextParams,
    ) -> RpcResult<()> {
        // check if device_session_id is available
        if let Some(device_session_id) = context_params.context.device_session_id {
            self.state
                .metrics
                .update_session_id(Some(device_session_id));
        }
        Ok(())
    }

    async fn remove_context(
        &self,
        _ctx: CallContext,
        request: MetricsContextKeys,
    ) -> RpcResult<()> {
        for key in request.keys {
            // currently handling only one key which is deviceSessionId
            if key.as_str() == "deviceSessionId" {
                self.state.metrics.update_session_id(None);
            }
        }
        Ok(())
    }
}

pub struct MetricsManagementProvider;
impl RippleRPCProvider<MetricsManagementImpl> for MetricsManagementProvider {
    fn provide(state: PlatformState) -> RpcModule<MetricsManagementImpl> {
        (MetricsManagementImpl { state }).into_rpc()
    }
}
