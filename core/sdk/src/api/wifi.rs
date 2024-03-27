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

use serde::{Deserialize, Serialize};

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

use super::device::device_wifi::{AccessPoint, AccessPointList};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WifiScanRequestTimeout {
    pub timeout: u64,
}

const DEFAULT_WIFI_SCAN_TIMEOUT: u64 = 60;

impl WifiScanRequestTimeout {
    pub fn new() -> Self {
        WifiScanRequestTimeout {
            timeout: DEFAULT_WIFI_SCAN_TIMEOUT,
        }
    }

    pub fn timeout(&self) -> u64 {
        self.timeout
    }
    pub fn set_timeout(&mut self, timeout: u64) {
        // use default if timeout is 0
        if timeout == 0 {
            self.timeout = DEFAULT_WIFI_SCAN_TIMEOUT;
        } else {
            self.timeout = timeout;
        }
    }
}

impl Default for WifiScanRequestTimeout {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum WifiResponse {
    CustomError(String),
    WifiScanListResponse(AccessPointList),
    WifiConnectSuccessResponse(AccessPoint),
}

impl ExtnPayloadProvider for WifiResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::Value(value)) = payload {
            if let Ok(v) = serde_json::from_value(value) {
                return Some(v);
            }
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Wifi
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::device::device_wifi::WifiSecurityMode;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_wifi_scan_request_timeout_new() {
        let timeout = WifiScanRequestTimeout::new();
        assert_eq!(timeout.timeout, DEFAULT_WIFI_SCAN_TIMEOUT);
    }

    #[test]
    fn test_wifi_scan_request_timeout_set_timeout() {
        let mut timeout = WifiScanRequestTimeout::new();
        timeout.set_timeout(30);
        assert_eq!(timeout.timeout, 30);

        timeout.set_timeout(0);
        assert_eq!(timeout.timeout, DEFAULT_WIFI_SCAN_TIMEOUT);
    }

    #[test]
    fn test_extn_payload_provider_for_wifi_response() {
        let access_point_list = AccessPointList {
            list: vec![AccessPoint {
                ssid: String::from("TestNetwork"),
                security_mode: WifiSecurityMode::Wpa2PskAes,
                signal_strength: -60,
                frequency: 2.4,
            }],
        };

        let wifi_response = WifiResponse::WifiScanListResponse(access_point_list);

        let contract_type: RippleContract = RippleContract::Wifi;
        test_extn_payload_provider(wifi_response, contract_type);
    }

    // Add more test cases as needed
}
