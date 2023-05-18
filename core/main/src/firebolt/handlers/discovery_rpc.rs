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
    RpcModule,
};
use ripple_sdk::{
    api::{
        apps::{AppMethod, AppRequest, AppResponse},
        firebolt::fb_discovery::{LaunchRequest, DISCOVERY_EVENT_ON_NAVIGATE_TO},
        gateway::rpc_gateway_api::CallContext,
    },
    log::info,
    tokio::sync::oneshot,
};

use crate::{
    firebolt::rpc::RippleRPCProvider, service::apps::app_events::AppEvents,
    state::platform_state::PlatformState, utils::rpc_utils::rpc_navigate_reserved_app_err,
};

#[rpc(server)]
pub trait Discovery {
    #[method(name = "discovery.launch")]
    async fn launch(&self, ctx: CallContext, request: LaunchRequest) -> RpcResult<bool>;
}

pub struct DiscoveryImpl {
    pub platform_state: PlatformState,
}

#[async_trait]
impl DiscoveryServer for DiscoveryImpl {
    async fn launch(&self, _ctx: CallContext, request: LaunchRequest) -> RpcResult<bool> {
        let app_defaults_configuration = self
            .platform_state
            .get_device_manifest()
            .applications
            .defaults;

        if let Some(reserved_app_id) =
            app_defaults_configuration.get_reserved_application_id(&request.app_id)
        {
            if reserved_app_id.is_empty() {
                return Err(rpc_navigate_reserved_app_err(
                    format!(
                        "Discovery.launch: Cannot find a valid reserved app id for {}",
                        request.app_id
                    )
                    .as_str(),
                ));
            }

            // Not validating the intent, pass-through to app as is.
            if !AppEvents::is_app_registered_for_event(
                &self.platform_state,
                reserved_app_id.to_string(),
                DISCOVERY_EVENT_ON_NAVIGATE_TO,
            ) {
                return Err(rpc_navigate_reserved_app_err(
                    format!("Discovery.launch: reserved app id {} is not registered for discovery.onNavigateTo event",
                    reserved_app_id).as_str(),
                ));
            }
            // emit EVENT_ON_NAVIGATE_TO to the reserved app.
            AppEvents::emit_to_app(
                &self.platform_state,
                reserved_app_id.to_string(),
                DISCOVERY_EVENT_ON_NAVIGATE_TO,
                &serde_json::to_value(request.intent).unwrap(),
            )
            .await;
            info!(
                "emit_to_app called for app {} event {}",
                reserved_app_id.to_string(),
                DISCOVERY_EVENT_ON_NAVIGATE_TO
            );
            return Ok(true);
        }
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request = AppRequest::new(AppMethod::Launch(request.clone()), app_resp_tx);

        if let Ok(_) = self
            .platform_state
            .get_client()
            .send_app_request(app_request)
        {
            if let Ok(_) = app_resp_rx.await {
                return Ok(true);
            }
        }

        Err(jsonrpsee::core::Error::Custom(String::from(
            "Discovery.launch: some failure",
        )))
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveryRpcProvider;
impl RippleRPCProvider<DiscoveryImpl> for DiscoveryRpcProvider {
    fn provide(platform_state: PlatformState) -> RpcModule<DiscoveryImpl> {
        (DiscoveryImpl { platform_state }).into_rpc()
    }
}
