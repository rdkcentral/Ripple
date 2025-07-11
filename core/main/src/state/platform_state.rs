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

use ripple_sdk::{
    api::{
        config::FEATURE_DISTRIBUTOR_SESSION,
        gateway::rpc_gateway_api::RpcRequest,
        manifest::{
            app_library::AppLibraryState,
            device_manifest::{AppLibraryEntry, DeviceManifest},
            exclusory::ExclusoryImpl,
            extn_manifest::ExtnManifest,
        },
        rules_engine::RuleEngineProvider,
        session::SessionAdjective,
    },
    extn::{
        extn_client_message::{ExtnMessage, ExtnPayloadProvider},
        extn_id::ExtnId,
    },
    framework::ripple_contract::RippleContract,
    utils::error::RippleError,
    uuid::Uuid,
};

use ssda_types::gateway::ApiGatewayServer;
use std::{collections::HashMap, fmt, sync::Arc};

use crate::{
    broker::endpoint_broker::EndpointBrokerState,
    firebolt::rpc_router::RouterState,
    service::{
        apps::{
            app_events::AppEventsState,
            delegated_launcher_handler::{AppManagerState, AppManagerState2_0},
            provider_broker::ProviderBrokerState,
        },
        extn::ripple_client::RippleClient,
        ripple_service::service_controller_state::ServiceControllerState,
    },
};

use super::{
    cap::cap_state::CapState, openrpc_state::OpenRpcState, ops_metrics_state::OpMetricState,
    ripple_cache::RippleCache, session_state::SessionState,
};

/// Platform state encapsulates the internal state of the Ripple Main application.
///
/// # Examples
/// ```
/// let state = PlatformState::default();
///
/// let manifest = state.get_device_manifest();
/// println!("{}", manifest.unwrap().configuration.platform);
/// ```
///

#[derive(Debug, Clone)]
pub struct DeviceSessionIdentifier {
    pub device_session_id: Uuid,
}

/// A wrapper for `Arc<tokio::sync::Mutex<Box<dyn ApiGatewayServer + Send + Sync>>>`
/// that implements the `Debug` trait.
#[derive(Clone)]
pub struct DebuggableApiGatewayServer(
    pub Arc<ripple_sdk::tokio::sync::Mutex<Box<dyn ApiGatewayServer + Send + Sync>>>,
);

impl fmt::Debug for DebuggableApiGatewayServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DebuggableApiGatewayServer")
    }
}

impl Default for DeviceSessionIdentifier {
    fn default() -> Self {
        Self {
            device_session_id: Uuid::new_v4(),
        }
    }
}
impl From<DeviceSessionIdentifier> for String {
    fn from(device_session_identifier: DeviceSessionIdentifier) -> Self {
        device_session_identifier.device_session_id.to_string()
    }
}
impl From<String> for DeviceSessionIdentifier {
    fn from(uuid_str: String) -> Self {
        DeviceSessionIdentifier {
            device_session_id: Uuid::parse_str(&uuid_str).unwrap_or_default(),
        }
    }
}

#[derive(Clone)]
pub struct PlatformState {
    pub extn_manifest: ExtnManifest,
    device_manifest: DeviceManifest,
    pub ripple_client: RippleClient,
    pub app_library_state: AppLibraryState,
    pub session_state: SessionState,
    pub cap_state: CapState,
    pub app_events_state: AppEventsState,
    pub provider_broker_state: ProviderBrokerState,
    pub app_manager_state: AppManagerState,
    pub open_rpc_state: OpenRpcState,
    pub router_state: RouterState,
    pub metrics: OpMetricState,
    pub device_session_id: DeviceSessionIdentifier,
    pub ripple_cache: RippleCache,
    pub version: Option<String>,
    pub endpoint_state: EndpointBrokerState,
    pub lifecycle2_app_state: AppManagerState2_0,
    pub service_controller_state: ServiceControllerState,
    pub services_gateway_api:
        Arc<ripple_sdk::tokio::sync::Mutex<Box<dyn ApiGatewayServer + Send + Sync>>>,
}
impl std::fmt::Debug for PlatformState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlatformState")
            .field("extn_manifest", &self.extn_manifest)
            .field("device_manifest", &self.device_manifest)
            .field("ripple_client", &self.ripple_client)
            .field("app_library_state", &self.app_library_state)
            .field("session_state", &self.session_state)
            .field("cap_state", &self.cap_state)
            .field("app_events_state", &self.app_events_state)
            .field("provider_broker_state", &self.provider_broker_state)
            .field("app_manager_state", &self.app_manager_state)
            .field("open_rpc_state", &self.open_rpc_state)
            .field("router_state", &self.router_state)
            .field("metrics", &self.metrics)
            .field("device_session_id", &self.device_session_id)
            .finish()
    }
}
#[cfg(test)]
impl Default for PlatformState {
    fn default() -> Self {
        use crate::state::bootstrap_state::ChannelsState;
        use ripple_sdk::api::rules_engine::RuleEngine;
        use ripple_sdk::tokio::sync::Mutex;
        use ripple_sdk::tokio::sync::RwLock;
        let extn_manifest = ExtnManifest::default();
        let rules_engine: Arc<RwLock<Box<dyn RuleEngineProvider + Send + Sync>>> =
            Arc::new(RwLock::new(Box::new(RuleEngine::build(&extn_manifest))));

        let api_gateway: Arc<Mutex<Box<dyn ApiGatewayServer + Send + Sync>>> =
            Arc::new(Mutex::new(Box::new(ssda_service::ApiGateway::new(
                rules_engine.clone(),
            ))));

        PlatformState::new(
            ExtnManifest::default(),
            DeviceManifest::default(),
            RippleClient::new(ChannelsState::default()),
            Vec::new(),
            None,
            api_gateway,
            rules_engine,
        )
    }
}
impl PlatformState {
    pub fn new(
        extn_manifest: ExtnManifest,
        manifest: DeviceManifest,
        client: RippleClient,
        app_library: Vec<AppLibraryEntry>,
        version: Option<String>,
        services_gateway_api: Arc<
            ripple_sdk::tokio::sync::Mutex<Box<dyn ApiGatewayServer + Send + Sync>>,
        >,
        rule_engine: Arc<
            ripple_sdk::tokio::sync::RwLock<Box<dyn RuleEngineProvider + Send + Sync>>,
        >,
    ) -> PlatformState {
        let exclusory = ExclusoryImpl::get(&manifest);
        let broker_sender = client.get_broker_sender();
        let extn_sdks = extn_manifest.extn_sdks.clone();
        let provider_registations = extn_manifest.provider_registrations.clone();
        let metrics_state = OpMetricState::default();
        Self {
            extn_manifest,
            cap_state: CapState::new(manifest.clone()),
            session_state: SessionState::default(),
            device_manifest: manifest.clone(),
            ripple_client: client.clone(),
            app_library_state: AppLibraryState::new(app_library),
            app_events_state: AppEventsState::default(),
            provider_broker_state: ProviderBrokerState::default(),
            app_manager_state: AppManagerState::new(&manifest.configuration.saved_dir),
            open_rpc_state: OpenRpcState::new(Some(exclusory), extn_sdks, provider_registations),
            router_state: RouterState::new(),
            metrics: metrics_state.clone(),
            device_session_id: DeviceSessionIdentifier::default(),
            ripple_cache: RippleCache::default(),
            version,
            endpoint_state: EndpointBrokerState::new(
                metrics_state,
                broker_sender,
                rule_engine,
                client,
            ),
            lifecycle2_app_state: AppManagerState2_0::new(),
            services_gateway_api: services_gateway_api.clone(),
            service_controller_state: ServiceControllerState::default(),
        }
    }

    pub fn has_internal_launcher(&self) -> bool {
        self.extn_manifest.get_launcher_capability().is_some()
    }

    pub fn get_launcher_capability(&self) -> Option<ExtnId> {
        self.extn_manifest.get_launcher_capability()
    }

    pub fn get_distributor_capability(&self) -> Option<ExtnId> {
        self.extn_manifest.get_distributor_capability()
    }

    pub fn get_manifest(&self) -> ExtnManifest {
        self.extn_manifest.clone()
    }

    pub fn get_rpc_aliases(&self) -> HashMap<String, Vec<String>> {
        self.extn_manifest.clone().rpc_aliases
    }

    pub fn get_device_manifest(&self) -> DeviceManifest {
        self.device_manifest.clone()
    }

    pub fn get_client(&self) -> RippleClient {
        self.ripple_client.clone()
    }

    pub async fn respond(&self, msg: ExtnMessage) -> Result<(), RippleError> {
        self.get_client().respond(msg).await
    }

    pub fn supports_distributor_session(&self) -> bool {
        self.get_client()
            .get_extn_client()
            .get_features()
            .contains(&String::from(FEATURE_DISTRIBUTOR_SESSION))
    }

    pub fn supports_session(&self) -> bool {
        let contract = RippleContract::Session(SessionAdjective::Account).as_clear_string();
        self.extn_manifest.required_contracts.contains(&contract)
    }

    pub fn supports_rfc(&self) -> bool {
        let contract = RippleContract::RemoteFeatureControl.as_clear_string();
        self.extn_manifest.required_contracts.contains(&contract)
    }
    ///
    /// War on dots
    pub async fn internal_rpc_request(
        &self,
        rpc_request: &RpcRequest,
    ) -> Result<ExtnMessage, RippleError> {
        self.get_client()
            .get_extn_client()
            .main_internal_request(rpc_request.to_owned())
            .await
    }
    ///
    /// Also war on dots
    pub async fn extn_request(
        &self,
        rpc_request: impl ExtnPayloadProvider,
    ) -> Result<ExtnMessage, RippleError> {
        self.get_client()
            .get_extn_client()
            .main_internal_request(rpc_request.to_owned())
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ripple_sdk::api::{manifest::extn_manifest::default_providers, rules_engine::RuleEngine};
    use ripple_tdk::utils::test_utils::Mockable;

    impl Mockable for PlatformState {
        fn mock() -> Self {
            use crate::state::bootstrap_state::ChannelsState;

            let (_, manifest) = DeviceManifest::load_from_content(
                include_str!("../../../../examples/manifest/device-manifest-example.json")
                    .to_string(),
            )
            .unwrap();
            let (_, mut extn_manifest) = ExtnManifest::load_from_content(
                include_str!("../../../../examples/manifest/extn-manifest-example.json")
                    .to_string(),
            )
            .unwrap();
            extn_manifest.provider_registrations = default_providers();
            let rules_engine: Arc<
                ripple_sdk::tokio::sync::RwLock<Box<dyn RuleEngineProvider + Send + Sync>>,
            > = Arc::new(ripple_sdk::tokio::sync::RwLock::new(Box::new(
                RuleEngine::build(&extn_manifest),
            )));

            let api_gateway_state: Arc<
                ripple_sdk::tokio::sync::Mutex<Box<dyn ApiGatewayServer + Send + Sync>>,
            > = Arc::new(ripple_sdk::tokio::sync::Mutex::new(Box::new(
                ssda_service::ApiGateway::new(rules_engine.clone()),
            )));
            Self::new(
                extn_manifest,
                manifest,
                RippleClient::new(ChannelsState::new()),
                vec![],
                None,
                api_gateway_state.clone(),
                rules_engine.clone(),
            )
        }
    }
}
