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

use thunder_ripple_sdk::{
    ripple_sdk::utils::error::RippleError, thunder_state::ThunderBootstrapStateWithClient,
};

use crate::processors::{
    thunder_browser::ThunderBrowserRequestProcessor,
    thunder_device_info::ThunderDeviceInfoRequestProcessor,
    thunder_persistent_store::ThunderStorageRequestProcessor,
    thunder_remote::ThunderRemoteAccessoryRequestProcessor,
    thunder_wifi::ThunderWifiRequestProcessor,
    thunder_window_manager::ThunderWindowManagerRequestProcessor,
};

pub struct SetupThunderProcessor;

impl SetupThunderProcessor {
    pub fn get_name() -> String {
        "SetupThunderProcessor".into()
    }

    pub async fn setup(
        state: ThunderBootstrapStateWithClient,
    ) -> Result<ThunderBootstrapStateWithClient, RippleError> {
        let mut extn_client = state.state.get_client();
        extn_client
            .add_request_processor(ThunderDeviceInfoRequestProcessor::new(state.clone().state));
        extn_client.add_request_processor(ThunderBrowserRequestProcessor::new(state.clone().state));
        extn_client.add_request_processor(ThunderWifiRequestProcessor::new(state.clone().state));
        extn_client.add_request_processor(ThunderStorageRequestProcessor::new(state.clone().state));
        extn_client.add_request_processor(ThunderWindowManagerRequestProcessor::new(
            state.clone().state,
        ));
        extn_client.add_request_processor(ThunderStorageRequestProcessor::new(state.clone().state));
        extn_client.add_request_processor(ThunderRemoteAccessoryRequestProcessor::new(
            state.clone().state,
        ));
        Ok(state)
    }
}
