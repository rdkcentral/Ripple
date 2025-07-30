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

use std::{collections::HashMap, pin::Pin};

use futures::Future;
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{ApiProtocol, CallContext, RpcRequest},
    extn::extn_client_message::ExtnResponse,
    log::info,
    utils::error::RippleError,
};
use serde_json::Value;
use std::sync::{Arc, Mutex};

use crate::state::platform_state::PlatformState;

type DecoratorFunctionType = Arc<
    dyn Fn(
            PlatformState,
            CallContext,
            Option<Value>,
        ) -> Pin<Box<dyn Future<Output = Result<Option<Value>, RippleError>> + Send>>
        + Send
        + Sync,
>;
pub struct EventManagementUtility {
    pub functions: Mutex<HashMap<String, DecoratorFunctionType>>,
}

impl Default for EventManagementUtility {
    fn default() -> Self {
        Self::new()
    }
}
impl EventManagementUtility {
    pub fn new() -> Self {
        Self {
            functions: Mutex::new(HashMap::new()),
        }
    }
    pub fn register_custom_functions(&self) {
        // Add a custom function to event utility based on the tag
        self.register_function(
            "AdvertisingPolicyEventDecorator".to_string(),
            Arc::new(|state, ctx, value| {
                let s = state.clone();
                Box::pin(EventManagementUtility::advertising_policy_event_decorator(
                    s, ctx, value,
                ))
            }),
        );
    }
    pub fn register_function(&self, name: String, function: DecoratorFunctionType) {
        self.functions.lock().unwrap().insert(name, function);
    }

    pub fn get_function(&self, name: &str) -> Option<DecoratorFunctionType> {
        self.functions.lock().unwrap().get(name).cloned()
    }

    pub async fn advertising_policy_event_decorator(
        platform_state: PlatformState,
        ctx: CallContext,
        value: Option<Value>,
    ) -> Result<Option<Value>, RippleError> {
        info!("advertising_policy_event_decorator called {:?}", value);

        let mut new_ctx = ctx.clone();
        new_ctx.protocol = ApiProtocol::Extn;

        let rpc_request = RpcRequest {
            ctx: new_ctx.clone(),
            method: "advertising.policy".into(),
            params_json: RpcRequest::prepend_ctx(None, &new_ctx),
        };

        platform_state
            .add_api_stats(&ctx.request_id, "advertising.policy")
            .await;

        let resp = platform_state
            .get_client()
            .get_extn_client()
            .main_internal_request(rpc_request.clone())
            .await;

        let policy = if let Ok(res) = resp.clone() {
            if let Some(ExtnResponse::Value(val)) = res.payload.extract::<ExtnResponse>() {
                Some(val)
            } else {
                None
            }
        } else {
            None
        };
        Ok(policy)
    }
}
