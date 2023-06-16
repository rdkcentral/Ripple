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
        apps::{AppMethod, AppRequest, AppResponse},
        device::entertainment_data::{LaunchIntent, NavigationIntent},
        firebolt::fb_discovery::{DiscoveryContext, LaunchRequest},
    },
    async_trait::async_trait,
    framework::{bootstrap::Bootstep, RippleResponse},
    tokio::sync::oneshot,
    utils::error::RippleError,
};

use crate::{
    processor::lifecycle_management_processor::LifecycleManagementProcessor,
    state::bootstrap_state::BootstrapState,
};

/// Bootstep which checks if the given run has the launcher channel and starts,
/// This step calls the start method on the Launcher Channel and waits for a successful Status
/// connection before proceeding to the next boot step.
pub struct CheckLauncherStep;

#[async_trait]
impl Bootstep<BootstrapState> for CheckLauncherStep {
    fn get_name(&self) -> String {
        "CheckLauncherStep".into()
    }
    async fn setup(&self, state: BootstrapState) -> RippleResponse {
        if state.platform_state.has_internal_launcher() {
            state.platform_state.get_client().add_request_processor(
                LifecycleManagementProcessor::new(state.platform_state.get_client()),
            );
            let app = state
                .platform_state
                .app_library_state
                .get_default_app()
                .expect("Default app to be available in app library");
            let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

            let app_request = AppRequest::new(
                AppMethod::Launch(LaunchRequest {
                    app_id: app.app_id,
                    intent: Some(NavigationIntent::Launch(LaunchIntent {
                        context: DiscoveryContext::new("device"),
                    })),
                }),
                app_resp_tx,
            );
            state
                .platform_state
                .get_client()
                .send_app_request(app_request)
                .expect("App Request to be sent successfully");

            if let Err(_) = app_resp_rx.await {
                return Err(RippleError::BootstrapError);
            }
        }
        Ok(())
    }
}
