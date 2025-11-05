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
    async_trait::async_trait, framework::bootstrap::Bootstep, utils::error::RippleError,
};

use crate::processor::metrics_processor::OpMetricsProcessor;
use crate::processor::settings_processor::SettingsProcessor;
use crate::{
    processor::{
        app_events_processor::AppEventsProcessor,
        authorized_info_processor::AuthorizedInfoProcessor,
        config_processor::ConfigRequestProcessor, keyboard_processor::KeyboardProcessor,
        pin_processor::PinProcessor, storage::storage_manager_processor::StorageManagerProcessor,
    },
    state::bootstrap_state::BootstrapState,
};

/// Sets up the SDK Extn Client and other components for IEC(Inter Extension Communication) clients are updated to app state for future use.
pub struct SetupExtnClientStep;

#[async_trait]
impl Bootstep<BootstrapState> for SetupExtnClientStep {
    fn get_name(&self) -> String {
        "SetupExtnClientStep".into()
    }
    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        let client = state.platform_state.get_client();
        // Main is now ready to take in config requests from extensions
        client.add_request_processor(ConfigRequestProcessor::new(state.platform_state.clone()));
        client.add_request_processor(PinProcessor::new(state.platform_state.clone()));
        client.add_request_processor(KeyboardProcessor::new(state.platform_state.clone()));
        client.add_event_processor(AppEventsProcessor::new(state.platform_state.clone()));
        client.add_request_processor(StorageManagerProcessor::new(state.platform_state.clone()));
        client.add_request_processor(AuthorizedInfoProcessor::new(state.platform_state.clone()));
        client.add_request_processor(SettingsProcessor::new(state.platform_state.clone()));
        client.add_request_processor(OpMetricsProcessor::new(state.platform_state.clone()));
        Ok(())
    }
}
