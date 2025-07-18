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
use std::{collections::HashMap, sync::Arc};

use crate::{
    broker::{endpoint_broker::EndpointBrokerState, rules::rules_engine::RuleEngine},
    firebolt::rpc_router::RouterState,
    service::{
        apps::{
            app_events::AppEventsState,
            delegated_launcher_handler::{AppManagerState, AppManagerState2_0},
            provider_broker::ProviderBrokerState,
        },
        extn::ripple_client::RippleClient,
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

#[derive(Debug, Clone)]
pub struct PlatformState {
    pub extn_manifest: Arc<ExtnManifest>,
    device_manifest: Arc<DeviceManifest>,
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
}

impl PlatformState {
    pub fn new(
        extn_manifest: ExtnManifest,
        manifest: DeviceManifest,
        client: RippleClient,
        app_library: Vec<AppLibraryEntry>,
        version: Option<String>,
    ) -> PlatformState {
        let exclusory = ExclusoryImpl::get(&manifest);
        let broker_sender = client.get_broker_sender();
        let rule_engine = RuleEngine::build(&extn_manifest);
        let extn_sdks = extn_manifest.extn_sdks.clone();
        let provider_registations = extn_manifest.provider_registrations.clone();
        let metrics_state = OpMetricState::default();
        Self {
            extn_manifest: Arc::new(extn_manifest),
            cap_state: CapState::new(manifest.clone()),
            session_state: SessionState::default(),
            device_manifest: Arc::new(manifest.clone()),
            ripple_client: client.clone(),
            app_library_state: AppLibraryState::new(app_library),
            app_events_state: AppEventsState::default(),
            provider_broker_state: ProviderBrokerState::default(),
            app_manager_state: AppManagerState::new(&manifest.configuration.saved_dir.clone()),
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
        (*self.extn_manifest).clone()
    }

    pub fn get_rpc_aliases(&self) -> HashMap<String, Vec<String>> {
        self.extn_manifest.rpc_aliases.clone()
    }

    pub fn get_device_manifest(&self) -> DeviceManifest {
        (*self.device_manifest).clone()
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
    use ripple_sdk::api::manifest::extn_manifest::default_providers;
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
            Self::new(
                extn_manifest,
                manifest,
                RippleClient::new(ChannelsState::new()),
                vec![],
                None,
            )
        }
    }
}
