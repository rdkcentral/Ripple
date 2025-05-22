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

pub mod client {
    pub mod device_operator;
    pub mod jsonrpc_method_locator;
    pub mod plugin_manager;
    pub mod thunder_async_client;
    pub mod thunder_async_client_plugins_status_mgr;
    pub mod thunder_client;
    pub mod thunder_plugin;
}

pub mod bootstrap {
    pub mod boot_thunder;
    pub mod setup_thunder_processors;
}

pub mod events {
    pub mod thunder_event_processor;
}

pub mod processors {
    pub mod thunder_device_info;
    pub mod thunder_events;
    pub mod events {
        pub mod thunder_event_handlers;
    }
    pub mod thunder_analytics;
    pub mod thunder_persistent_store;
    pub mod thunder_rfc;
    pub mod thunder_telemetry;
}

pub mod utils;

pub mod thunder_state;
pub extern crate ripple_sdk;

#[cfg(any(test, feature = "mock"))]
pub mod tests {
    #[cfg(feature = "websocket_contract_tests")]
    pub mod contracts {
        pub mod contract_utils;
        pub mod thunder_controller_pacts;
        pub mod thunder_device_info_pacts;
        pub mod thunder_persistent_store_pacts;
        pub mod thunder_text_track_pacts;
        pub mod thunder_user_settings_pacts;
    }
    pub mod mock_thunder_controller;
    pub mod thunder_client_pool_test_utility;
}
