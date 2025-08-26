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
use ripple_sdk::{
    api::{
        apps::{AppEvent, AppManagerResponse, AppMethod, AppRequest, AppResponse},
        caps::CapsRequest,
        firebolt::{fb_discovery::AgePolicy, fb_general::ListenRequestWithEvent, fb_telemetry::TelemetryPayload},
        gateway::rpc_gateway_api::CallContext,
    },
    async_trait::async_trait,
    log::{debug, error},
    tokio::sync::oneshot,
};
use std::{collections::HashMap, sync::{Arc, RwLock}};
use serde::{Serialize, Deserialize};

use crate::{
    firebolt::rpc::RippleRPCProvider,
    service::{apps::app_events::AppEvents, telemetry_builder::TelemetryBuilder},
    state::platform_state::PlatformState,
    utils::rpc_utils::rpc_await_oneshot,
};

#[rpc(server)]
pub trait Internal {
    #[method(name = "ripple.sendTelemetry")]
    async fn send_telemetry(&self, ctx: CallContext, payload: TelemetryPayload) -> RpcResult<()>;

    #[method(name = "ripple.setTelemetrySessionId")]
    fn set_telemetry_session_id(&self, ctx: CallContext, session_id: String) -> RpcResult<()>;

    #[method(name = "ripple.sendAppEvent")]
    async fn send_app_event(&self, ctx: CallContext, event: AppEvent) -> RpcResult<()>;

    #[method(name = "ripple.registerAppEvent")]
    async fn register_app_event(
        &self,
        ctx: CallContext,
        request: ListenRequestWithEvent,
    ) -> RpcResult<()>;

    #[method(name = "ripple.getAppCatalogId")]
    async fn get_app_catalog_id(&self, ctx: CallContext, app_id: String) -> RpcResult<String>;

    #[method(name = "ripple.checkCapsRequest")]
    async fn check_caps_request(
        &self,
        ctx: CallContext,
        caps_request: CapsRequest,
    ) -> RpcResult<HashMap<String, bool>>;
    
    #[method(name = "account.policyIdentifierAlias")]
    async fn set_policy_identifier_alias(
        &self,
        ctx: CallContext,
        request: PolicyIdentifierAlias,
    ) -> RpcResult<()>;
}

#[derive(Debug, Clone, Default)]
pub struct PolicyState {
    pub policy_identifiers_alias: Arc<RwLock<Vec<AgePolicy>>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PolicyIdentifierAlias {
    #[serde(rename = "policyIdentifierAlias")]
    pub policy_identifier_alias: Vec<AgePolicy>,
}
#[derive(Debug)]
pub struct InternalImpl {
    pub state: PlatformState,
}

#[async_trait]
impl InternalServer for InternalImpl {
    async fn send_telemetry(&self, _ctx: CallContext, payload: TelemetryPayload) -> RpcResult<()> {
        let _ = TelemetryBuilder::send_telemetry(&self.state, payload);
        Ok(())
    }

    fn set_telemetry_session_id(&self, _ctx: CallContext, session_id: String) -> RpcResult<()> {
        self.state.metrics.update_session_id(Some(session_id));
        Ok(())
    }

    async fn send_app_event(&self, _ctx: CallContext, event: AppEvent) -> RpcResult<()> {
        debug!("Sending App event {:?}", &event);
        AppEvents::emit_with_context(&self.state, &event.event_name, &event.result, event.context)
            .await;
        Ok(())
    }

    async fn register_app_event(
        &self,
        _ctx: CallContext,
        request: ListenRequestWithEvent,
    ) -> RpcResult<()> {
        debug!("registering App event {:?}", &request);
        let event = request.event.clone();
        AppEvents::add_listener(&self.state, event, request.context.clone(), request.request);
        Ok(())
    }

    async fn get_app_catalog_id(&self, _: CallContext, app_id: String) -> RpcResult<String> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request =
            AppRequest::new(AppMethod::GetAppContentCatalog(app_id.clone()), app_resp_tx);
        if let Err(e) = self.state.get_client().send_app_request(app_request) {
            error!("Send error for AppMethod::GetAppContentCatalog {:?}", e);
        }
        let resp = rpc_await_oneshot(app_resp_rx).await;

        if let Ok(Ok(AppManagerResponse::AppContentCatalog(content_catalog))) = resp {
            return Ok(content_catalog.map_or(app_id.to_owned(), |x| x));
        }

        Ok(app_id)
    }

    async fn check_caps_request(
        &self,
        _ctx: CallContext,
        caps_request: CapsRequest,
    ) -> RpcResult<HashMap<String, bool>> {
        match caps_request {
            CapsRequest::Supported(request) => {
                let result = self.state.cap_state.generic.check_for_processor(request);
                Ok(result)
            }
            CapsRequest::Permitted(app_id, request) => {
                let result = self
                    .state
                    .cap_state
                    .permitted_state
                    .check_multiple(&app_id, request);
                Ok(result)
            }
        }
    }

    async fn set_policy_identifier_alias(&self, ctx: CallContext, policy_identifier_alias: PolicyIdentifierAlias) -> RpcResult<()> {
        let mut policy_state = self.state.policy_state.clone();
        let _result = policy_state.add_policy_identifier_alias(policy_identifier_alias).await;
        Ok(())
    }
}

pub struct InternalProvider;
impl RippleRPCProvider<InternalImpl> for InternalProvider {
    fn provide(state: PlatformState) -> RpcModule<InternalImpl> {
        (InternalImpl { state }).into_rpc()
    }
}

impl PolicyState {
    pub async fn add_policy_identifier_alias(
    &mut self,
    policy: PolicyIdentifierAlias  ,
    ) {
        let mut policy_identifiers_alias = self.policy_identifiers_alias.write().unwrap();
        if policy.policy_identifier_alias.is_empty() {
            policy_identifiers_alias.clear();
            return
        }
        for identifier in policy.policy_identifier_alias {
            if !policy_identifiers_alias.contains(&identifier) {
                policy_identifiers_alias.push(identifier);
            } 
        }
        debug!("Updated policy identifiers alias: {:?}", &policy_identifiers_alias);
    }
}
