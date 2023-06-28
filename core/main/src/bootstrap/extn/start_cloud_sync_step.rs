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
use ripple_sdk::{
    api::distributor::distributor_sync::{SyncAndMonitorModule, SyncAndMonitorRequest},
    framework::bootstrap::Bootstep,
};
use ripple_sdk::{async_trait::async_trait, framework::RippleResponse};

use crate::state::bootstrap_state::BootstrapState;

use ripple_sdk::log::debug;
pub struct StartCloudSyncStep;

#[async_trait]
impl Bootstep<BootstrapState> for StartCloudSyncStep {
    fn get_name(&self) -> String {
        "StartCloudSyncStep".into()
    }

    async fn setup(&self, state: BootstrapState) -> RippleResponse {
        if !state.platform_state.supports_cloud_sync() {
            debug!("Cloud Sync not configured as a required contract so not starting.");
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
