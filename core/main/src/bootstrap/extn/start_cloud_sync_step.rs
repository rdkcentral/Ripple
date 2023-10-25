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
        distributor::distributor_sync::{SyncAndMonitorModule, SyncAndMonitorRequest},
        manifest::device_manifest::PrivacySettingsStorageType,
    },
    framework::bootstrap::Bootstep,
    tokio,
};
use ripple_sdk::{async_trait::async_trait, framework::RippleResponse};

use crate::{service::data_governance::DataGovernance, state::bootstrap_state::BootstrapState};

use std::time::Duration;

use ripple_sdk::log::debug;
pub struct StartCloudSyncStep;

impl StartCloudSyncStep {
    async fn sync_partner_exclusions(state: &BootstrapState) {
        let state_for_exclusion = state.platform_state.clone();
        tokio::spawn(async move {
            let duration = state_for_exclusion
                .get_device_manifest()
                .configuration
                .partner_exclusion_refresh_timeout
                .into();
            let mut interval = tokio::time::interval(Duration::from_secs(duration));
            loop {
                let resp: bool =
                    DataGovernance::refresh_partner_exclusions(&state_for_exclusion).await;
                debug!("refresh_partner_exclusions: {:?}", resp);
                interval.tick().await;
            }
        });
    }
}
#[async_trait]
impl Bootstep<BootstrapState> for StartCloudSyncStep {
    fn get_name(&self) -> String {
        "StartCloudSyncStep".into()
    }

    async fn setup(&self, state: BootstrapState) -> RippleResponse {
        //sync up partner exclusion data and setup polling thread for refreshing it.
        Self::sync_partner_exclusions(&state).await;
        if !state.platform_state.supports_cloud_sync() {
            debug!("Cloud Sync not configured as a required contract so not starting.");
            return Ok(());
        }
        if state
            .platform_state
            .get_device_manifest()
            .configuration
            .features
            .privacy_settings_storage_type
            != PrivacySettingsStorageType::Sync
        {
            debug!(
                "Privacy settings storage type is not set as sync so not starting cloud monitor"
            );
            return Ok(());
        }
        if let Some(account_session) = state.platform_state.session_state.get_account_session() {
            debug!("Successfully got account session");
            let sync_response = state
                .platform_state
                .get_client()
                .send_extn_request(SyncAndMonitorRequest::SyncAndMonitor(
                    SyncAndMonitorModule::Privacy,
                    account_session.clone(),
                ))
                .await;
            debug!("Received Sync response for privacy: {:?}", sync_response);
            let sync_response = state
                .platform_state
                .get_client()
                .send_extn_request(SyncAndMonitorRequest::SyncAndMonitor(
                    SyncAndMonitorModule::UserGrants,
                    account_session.clone(),
                ))
                .await;
            debug!(
                "Received Sync response for user grants: {:?}",
                sync_response
            );
        }

        Ok(())
    }
}
