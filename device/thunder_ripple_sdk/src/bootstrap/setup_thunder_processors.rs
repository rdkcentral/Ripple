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

use ripple_sdk::api::firebolt::fb_telemetry::OperationalMetricRequest;
use ripple_sdk::api::status_update::ExtnStatus;
use ripple_sdk::log::error;

use crate::processors::thunder_package_manager::ThunderPackageManagerRequestProcessor;
use crate::processors::thunder_rfc::ThunderRFCProcessor;
use crate::processors::thunder_telemetry::ThunderTelemetryProcessor;
use crate::thunder_state::ThunderBootstrapStateWithClient;

use crate::processors::{
    thunder_browser::ThunderBrowserRequestProcessor,
    thunder_device_info::ThunderDeviceInfoRequestProcessor,
    thunder_events::ThunderOpenEventsProcessor,
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

    pub async fn setup(state: ThunderBootstrapStateWithClient) {
        let mut extn_client = state.state.get_client();
        extn_client
            .add_request_processor(ThunderDeviceInfoRequestProcessor::new(state.clone().state));
        extn_client.add_request_processor(ThunderBrowserRequestProcessor::new(state.clone().state));
        extn_client.add_request_processor(ThunderWifiRequestProcessor::new(state.clone().state));
        extn_client.add_request_processor(ThunderStorageRequestProcessor::new(state.clone().state));
        extn_client.add_request_processor(ThunderWindowManagerRequestProcessor::new(
            state.state.clone(),
        ));
        extn_client.add_request_processor(ThunderRemoteAccessoryRequestProcessor::new(
            state.clone().state,
        ));
        extn_client.add_request_processor(ThunderOpenEventsProcessor::new(state.clone().state));

        let package_manager_processor =
            ThunderPackageManagerRequestProcessor::new(state.clone().state);
        extn_client.add_request_processor(package_manager_processor);

        if extn_client.get_bool_config("rdk_telemetry") {
            match extn_client
                .request(OperationalMetricRequest::Subscribe)
                .await
            {
                Ok(_) => extn_client
                    .add_event_processor(ThunderTelemetryProcessor::new(state.clone().state)),
                Err(_) => error!("Telemetry not setup"),
            }
        }
        extn_client.add_request_processor(ThunderRFCProcessor::new(state.clone().state));
        let _ = extn_client.event(ExtnStatus::Ready);
    }
}
