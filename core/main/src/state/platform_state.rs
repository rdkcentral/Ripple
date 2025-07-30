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
        observability::metrics_util::ApiStats,
        session::SessionAdjective,
    },
    chrono::{DateTime, Utc},
    extn::{
        extn_client_message::{ExtnMessage, ExtnPayloadProvider},
        extn_id::ExtnId,
    },
    framework::ripple_contract::RippleContract,
    tokio::{self, sync::RwLock},
    utils::{error::RippleError, sync::AsyncShared},
    uuid::Uuid,
};
use std::{collections::HashMap, future::Future, sync::Arc};

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

#[derive(Debug, Clone, Default)]
pub struct PlatformStateContainer {
    pub extn_manifest: Arc<ExtnManifest>,
    device_manifest: Arc<DeviceManifest>,
    pub ripple_client: Arc<RippleClient>,
    pub app_library_state: Arc<AppLibraryState>,
    pub session_state: Arc<SessionState>,
    pub cap_state: Arc<CapState>,
    pub app_events_state: Arc<AppEventsState>,
    pub provider_broker_state: Arc<ProviderBrokerState>,
    pub app_manager_state: Arc<AppManagerState>,
    pub open_rpc_state: Arc<OpenRpcState>,
    pub router_state: Arc<RouterState>,
    metrics: AsyncShared<OpMetricState>,
    pub device_session_id: DeviceSessionIdentifier,
    ripple_cache: AsyncShared<RippleCache>,
    pub version: Option<String>,
    endpoint_state: AsyncShared<EndpointBrokerState>,
    pub lifecycle2_app_state: Arc<AppManagerState2_0>,
}

impl PlatformStateContainer {
    pub fn new_instance(
        extn_manifest: ExtnManifest,
        manifest: DeviceManifest,
        client: RippleClient,
        app_library: Vec<AppLibraryEntry>,
        version: Option<String>,
    ) -> PlatformStateContainer {
        let exclusory = ExclusoryImpl::get(&manifest);
        let broker_sender = client.get_broker_sender();
        let rule_engine = RuleEngine::build(&extn_manifest);
        let extn_sdks = extn_manifest.extn_sdks.clone();
        let provider_registations = extn_manifest.provider_registrations.clone();
        let metrics_state = Arc::new(RwLock::new(OpMetricState::default()));
        Self {
            extn_manifest: Arc::new(extn_manifest),
            cap_state: CapState::new(manifest.clone()).into(),
            session_state: Arc::new(SessionState::default()),
            device_manifest: Arc::new(manifest.clone()),
            ripple_client: client.clone().into(),
            app_library_state: Arc::new(AppLibraryState::new(app_library)),
            app_events_state: Arc::new(AppEventsState::default()),
            provider_broker_state: Arc::new(ProviderBrokerState::default()),
            app_manager_state: AppManagerState::new(&manifest.configuration.saved_dir.clone())
                .into(),
            open_rpc_state: OpenRpcState::new(Some(exclusory), extn_sdks, provider_registations),
            router_state: Arc::new(RouterState::new()),
            metrics: metrics_state.clone(),
            device_session_id: DeviceSessionIdentifier::default(),
            ripple_cache: Arc::new(tokio::sync::RwLock::new(RippleCache::default())),
            version,

            endpoint_state: Arc::new(tokio::sync::RwLock::new(EndpointBrokerState::new(
                broker_sender,
                rule_engine,
                client,
            ))),

            lifecycle2_app_state: Arc::new(AppManagerState2_0::new()),
        }
    }
    pub fn new(
        extn_manifest: ExtnManifest,
        manifest: DeviceManifest,
        client: RippleClient,
        app_library: Vec<AppLibraryEntry>,
        version: Option<String>,
    ) -> PlatformState {
        Arc::new(Self::new_instance(
            extn_manifest,
            manifest,
            client,
            app_library,
            version,
        ))
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

    pub fn get_client(&self) -> Arc<RippleClient> {
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

    /*
    accessors to attempt to jail access
     */
    pub fn extn_manifest(&self) -> Arc<ExtnManifest> {
        self.extn_manifest.clone()
    }

    pub fn device_manifest(&self) -> Arc<DeviceManifest> {
        self.device_manifest.clone()
    }

    pub fn ripple_client(&self) -> Arc<RippleClient> {
        self.ripple_client.clone()
    }

    pub fn app_library_state(&self) -> Arc<AppLibraryState> {
        self.app_library_state.clone()
    }

    pub fn session_state(&self) -> Arc<SessionState> {
        self.session_state.clone()
    }

    pub fn cap_state(&self) -> Arc<CapState> {
        self.cap_state.clone()
    }

    pub fn app_events_state(&self) -> Arc<AppEventsState> {
        self.app_events_state.clone()
    }

    pub fn provider_broker_state(&self) -> Arc<ProviderBrokerState> {
        self.provider_broker_state.clone()
    }

    pub fn app_manager_state(&self) -> Arc<AppManagerState> {
        self.app_manager_state.clone()
    }

    pub fn open_rpc_state(&self) -> Arc<OpenRpcState> {
        self.open_rpc_state.clone()
    }

    pub fn router_state(&self) -> Arc<RouterState> {
        self.router_state.clone()
    }

    pub fn _metrics(&self) -> Arc<RwLock<OpMetricState>> {
        self.metrics.clone()
    }

    pub fn device_session_id(&self) -> &DeviceSessionIdentifier {
        &self.device_session_id
    }

    pub fn version(&self) -> Option<String> {
        self.version.clone()
    }

    pub fn lifecycle2_app_state(&self) -> Arc<AppManagerState2_0> {
        self.lifecycle2_app_state.clone()
    }

    pub async fn endpoint_state<R, F, Fut>(&self, f: F) -> R
    where
        F: FnOnce(EndpointBrokerState) -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let guard = self.endpoint_state.read().await;
        let cloned = guard.clone(); // ✅ requires Clone
        drop(guard); // ✅ release lock before `.await`
        f(cloned).await
    }
    /// For sync usage (new version)
    pub async fn endpoint_state_sync<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&EndpointBrokerState) -> R,
    {
        let guard = self.endpoint_state.read().await; // sync read
        f(&guard)
    }
    pub async fn endpoint_state_sync_write<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut EndpointBrokerState) -> R,
    {
        let mut guard = self.endpoint_state.write().await;
        f(&mut guard)
    }
    pub async fn endpoint_state_write<R, F, Fut>(&self, f: F) -> R
    where
        F: FnOnce(EndpointBrokerState) -> Fut, // take ownership
        Fut: Future<Output = R>,
    {
        let guard = self.endpoint_state.write().await;
        let cloned = guard.clone();
        drop(guard);
        f(cloned).await
    }
    pub async fn ripple_cache<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&RippleCache) -> R,
    {
        let guard = self.ripple_cache.read().await;
        f(&guard)
    }

    // Mutable async access to ripple_cache
    pub async fn ripple_cache_write<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut RippleCache) -> R,
    {
        let mut guard = self.ripple_cache.write().await;
        f(&mut guard)
    }
    pub async fn add_api_stats(&self, request_id: &str, method: &str) {
        let metrics = self.metrics.write().await;
        metrics.add_api_stats(request_id, method).await;
    }
    pub async fn update_api_stage(&self, request_id: &str, stage: &str) -> i64 {
        let mut metrics = self.metrics.write().await;
        metrics.update_api_stage(request_id, stage).await
    }

    pub async fn update_api_stats_ref(&self, request_id: &str, stats_ref: Option<String>) {
        let mut metrics = self.metrics.write().await;
        metrics.update_api_stats_ref(request_id, stats_ref).await;
    }
    pub async fn remove_api_stats(&self, request_id: &str) {
        let mut metrics = self.metrics.write().await;
        metrics.remove_api_stats(request_id).await
    }

    pub async fn get_device_session_id(&self) -> String {
        let metrics = self.metrics.read().await;
        metrics.get_device_session_id().await
    }

    pub async fn update_session_id(&self, value: Option<String>) {
        let metrics = self.metrics.write().await;
        let _ = metrics.update_session_id(value).await;
    }

    pub async fn get_listeners(&self) -> Vec<String> {
        let metrics = self.metrics.read().await;
        metrics.get_listeners().await
    }

    pub async fn operational_telemetry_listener(&self, target: &str, listen: bool) {
        let metrics = self.metrics.write().await;
        metrics.operational_telemetry_listener(target, listen).await;
    }
    pub async fn get_metrics_start_time(&self) -> DateTime<Utc> {
        let metrics = self.metrics.read().await;
        metrics.get_start_time()
    }
    pub async fn get_api_stats(&self, request_id: &str) -> Option<ApiStats> {
        let metrics = self.metrics.read().await;
        metrics.get_api_stats(request_id).await
    }

    pub async fn metrics<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&OpMetricState) -> R,
    {
        let guard = self.metrics.read().await;
        f(&guard)
    }

    // Mutable async access to metrics
    pub async fn metrics_mut<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut OpMetricState) -> R,
    {
        let mut guard = self.metrics.write().await;
        f(&mut guard)
    }

    // Optional sync variant for testing or CLI tools
    #[cfg(test)]
    pub fn metrics_sync_mut<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut OpMetricState) -> R,
    {
        if tokio::runtime::Handle::try_current().is_ok() {
            panic!("Cannot use blocking_write inside async runtime");
        }
        let mut guard = self.metrics.blocking_write();
        f(&mut guard)
    }
}
#[cfg(test)]
use ripple_sdk::utils::sync::AsyncRwLock;

#[cfg(test)]
#[derive(Default)]
pub struct PlatformStateContainerBuilder {
    extn_manifest: Option<Arc<ExtnManifest>>,
    device_manifest: Option<Arc<DeviceManifest>>,
    ripple_client: Option<Arc<RippleClient>>,
    app_library_state: Option<Arc<AppLibraryState>>,
    session_state: Option<Arc<SessionState>>,
    cap_state: Option<Arc<CapState>>,
    app_events_state: Option<Arc<AppEventsState>>,
    provider_broker_state: Option<Arc<ProviderBrokerState>>,
    app_manager_state: Option<Arc<AppManagerState>>,
    open_rpc_state: Option<Arc<OpenRpcState>>,
    router_state: Option<Arc<RouterState>>,
    metrics: Option<Arc<RwLock<OpMetricState>>>,
    device_session_id: Option<DeviceSessionIdentifier>,
    ripple_cache: Option<Arc<AsyncRwLock<RippleCache>>>,
    version: Option<String>,
    endpoint_state: Option<AsyncShared<EndpointBrokerState>>,
    lifecycle2_app_state: Option<Arc<AppManagerState2_0>>,
}

#[cfg(test)]
impl PlatformStateContainerBuilder {
    pub fn new() -> Self {
        Self {
            extn_manifest: None,
            device_manifest: None,
            ripple_client: None,
            app_library_state: None,
            session_state: None,
            cap_state: None,
            app_events_state: None,
            provider_broker_state: None,
            app_manager_state: None,
            open_rpc_state: None,
            router_state: None,
            metrics: None,
            device_session_id: None,
            ripple_cache: None,
            version: None,
            endpoint_state: None,
            lifecycle2_app_state: None,
        }
    }

    pub fn extn_manifest(mut self, value: Arc<ExtnManifest>) -> Self {
        self.extn_manifest = Some(value);
        self
    }
    pub fn device_manifest(mut self, value: Arc<DeviceManifest>) -> Self {
        self.device_manifest = Some(value);
        self
    }
    pub fn ripple_client(mut self, value: Arc<RippleClient>) -> Self {
        self.ripple_client = Some(value);
        self
    }
    pub fn app_library_state(mut self, value: Arc<AppLibraryState>) -> Self {
        self.app_library_state = Some(value);
        self
    }
    pub fn session_state(mut self, value: Arc<SessionState>) -> Self {
        self.session_state = Some(value);
        self
    }
    pub fn cap_state(mut self, value: Arc<CapState>) -> Self {
        self.cap_state = Some(value);
        self
    }
    pub fn app_events_state(mut self, value: Arc<AppEventsState>) -> Self {
        self.app_events_state = Some(value);
        self
    }
    pub fn provider_broker_state(mut self, value: Arc<ProviderBrokerState>) -> Self {
        self.provider_broker_state = Some(value);
        self
    }
    pub fn app_manager_state(mut self, value: Arc<AppManagerState>) -> Self {
        self.app_manager_state = Some(value);
        self
    }
    pub fn open_rpc_state(mut self, value: Arc<OpenRpcState>) -> Self {
        self.open_rpc_state = Some(value);
        self
    }
    pub fn router_state(mut self, value: Arc<RouterState>) -> Self {
        self.router_state = Some(value);
        self
    }
    pub fn metrics(mut self, value: Arc<RwLock<OpMetricState>>) -> Self {
        self.metrics = Some(value);
        self
    }
    pub fn device_session_id(mut self, value: DeviceSessionIdentifier) -> Self {
        self.device_session_id = Some(value);
        self
    }
    pub fn ripple_cache(mut self, value: Arc<AsyncRwLock<RippleCache>>) -> Self {
        self.ripple_cache = Some(value);
        self
    }
    pub fn version(mut self, value: Option<String>) -> Self {
        self.version = value;
        self
    }
    pub fn endpoint_state(mut self, value: AsyncShared<EndpointBrokerState>) -> Self {
        self.endpoint_state = Some(value);
        self
    }
    pub fn lifecycle2_app_state(mut self, value: Arc<AppManagerState2_0>) -> Self {
        self.lifecycle2_app_state = Some(value);
        self
    }

    pub fn build(self) -> PlatformStateContainer {
        PlatformStateContainer {
            extn_manifest: self
                .extn_manifest
                .unwrap_or_else(|| Arc::new(Default::default())),
            device_manifest: self
                .device_manifest
                .unwrap_or_else(|| Arc::new(Default::default())),
            ripple_client: self
                .ripple_client
                .unwrap_or_else(|| Arc::new(Default::default())),
            app_library_state: self
                .app_library_state
                .unwrap_or_else(|| Arc::new(Default::default())),
            session_state: self
                .session_state
                .unwrap_or_else(|| Arc::new(Default::default())),
            cap_state: self
                .cap_state
                .unwrap_or_else(|| Arc::new(Default::default())),
            app_events_state: self
                .app_events_state
                .unwrap_or_else(|| Arc::new(Default::default())),
            provider_broker_state: self
                .provider_broker_state
                .unwrap_or_else(|| Arc::new(Default::default())),
            app_manager_state: self
                .app_manager_state
                .unwrap_or_else(|| Arc::new(Default::default())),
            open_rpc_state: self
                .open_rpc_state
                .unwrap_or_else(|| Arc::new(Default::default())),
            router_state: self
                .router_state
                .unwrap_or_else(|| Arc::new(Default::default())),
            metrics: self
                .metrics
                .unwrap_or_else(|| Arc::new(RwLock::new(Default::default()))),
            device_session_id: self.device_session_id.unwrap_or_default(),
            ripple_cache: self
                .ripple_cache
                .unwrap_or_else(|| Arc::new(AsyncRwLock::new(Default::default()))),
            version: self.version,
            endpoint_state: self.endpoint_state.unwrap_or_default(),
            lifecycle2_app_state: self
                .lifecycle2_app_state
                .unwrap_or_else(|| Arc::new(Default::default())),
        }
    }
}

pub type PlatformState = std::sync::Arc<PlatformStateContainer>;

#[cfg(test)]
mod tests {
    use super::*;
    use ripple_sdk::api::manifest::extn_manifest::default_providers;
    use ripple_tdk::utils::test_utils::Mockable;

    impl Mockable for PlatformStateContainer {
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
            Self::new_instance(
                extn_manifest,
                manifest,
                RippleClient::new(ChannelsState::new()),
                vec![],
                None,
            )
        }
    }
}
