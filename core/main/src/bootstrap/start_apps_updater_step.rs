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

use ripple_sdk::api::app_catalog::AppCatalogRequest;
use ripple_sdk::async_trait::async_trait;
use ripple_sdk::{framework::bootstrap::Bootstep, utils::error::RippleError};

use crate::service::apps::apps_updater::AppsUpdater;
use crate::state::bootstrap_state::BootstrapState;

/// Starts the Apps updater
pub struct StartAppsUpdaterStep;

#[async_trait]
impl Bootstep<BootstrapState> for StartAppsUpdaterStep {
    fn get_name(&self) -> String {
        "StartAppsUpdater".into()
    }

    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        let ignore_list = vec![
            state
                .platform_state
                .get_device_manifest()
                .applications
                .defaults
                .main,
        ];
        let client = state.platform_state.get_client().clone();
        client.add_event_processor(AppsUpdater::new(client.get_extn_client(), ignore_list));

        match state
            .platform_state
            .ripple_client
            .send_extn_request(AppCatalogRequest::OnAppsUpdate)
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(RippleError::BootstrapError),
        }
    }
}
