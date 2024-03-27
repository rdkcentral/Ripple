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

use super::device_request::DeviceRequest;
use crate::{
    api::{firebolt::fb_capabilities::FireboltCap, manifest::device_manifest::DeviceManifest},
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum RemoteAccessoryRequest {
    Pair(AccessoryPairRequest),
    List(AccessoryListRequest),
}

impl ExtnPayloadProvider for RemoteAccessoryRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Accessory(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Accessory(d))) = payload {
            return Some(d);
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::RemoteAccessory
    }
}

#[async_trait]
pub trait AccessoryService {
    async fn pair(self: Box<Self>, pair_request: Option<AccessoryPairRequest>) -> Box<Self>;

    async fn list(self: Box<Self>, list_request: Option<AccessoryListRequest>) -> Box<Self>;
}

/// Constructs a request to pair an accessory.
///
/// # Examples
/// Note the device needs to support the AccessoryType and Pairing Protocol.
/// ```
/// use ripple_sdk::api::device::device_accessory::{AccessoryType,AccessoryProtocol,AccessoryPairRequest};
/// AccessoryPairRequest{_type: AccessoryType::Remote, protocol: AccessoryProtocol::BluetoothLE , timeout: 180};
///

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AccessoryPairRequest {
    #[serde(rename = "type")]
    pub _type: AccessoryType,
    pub protocol: AccessoryProtocol,
    pub timeout: u64,
}

impl Default for AccessoryPairRequest {
    fn default() -> Self {
        AccessoryPairRequest {
            _type: AccessoryType::Other,
            protocol: AccessoryProtocol::BluetoothLE,
            timeout: 60,
        }
    }
}

/// Enumeration to support various Accessories which can be paired to the device.
///
/// # More info
///
/// ```
/// use ripple_sdk::api::device::device_accessory::AccessoryType;
/// AccessoryType::Remote; // Remote device used to send keypress events to the device
/// AccessoryType::Speaker; // Audio output device which recieves sound signals from device.
/// AccessoryType::Other; // Placeholder for other types of supported devices. Subjected to platform support.
/// ```
///
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash, Eq)]
pub enum AccessoryType {
    Remote,
    Speaker,
    Other,
}

/// Enumeration to enlist the different query params for a given device supported on a Accessory List request.
///
/// # More info
///
/// ```
/// use ripple_sdk::api::device::device_accessory::AccessoryListType;
/// AccessoryListType::Remote; // List of remotes connected to the Device
/// AccessoryListType::Speaker; // List of Speakers connected to the Device.
/// AccessoryListType::All; // All Paired accesories connected to the Device.
/// ```
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AccessoryListType {
    Remote,
    Speaker,
    All,
}

/// Enumeration to enlist the different query params based on the connected protocol on a Accessory List request.
///
/// # More info
///
/// ```
/// use ripple_sdk::api::device::device_accessory::AccessoryProtocolListType;
/// AccessoryProtocolListType::BluetoothLE; // List of Devices connected via BluetoothLe
/// AccessoryProtocolListType::RF4CE; // List of Devices connected via RF4CE(Radio Frequency).
/// AccessoryProtocolListType::All; // All Paired accesories connected to the Device.
/// ```
///
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AccessoryProtocolListType {
    BluetoothLE,
    RF4CE,
    All,
}

/// Constructs a request to list accessories.
///
/// # Examples
/// Note the device needs to support the AccessoryType and Pairing Protocol.
///
/// List all Bluetooth remotes
/// ```
/// use ripple_sdk::api::device::device_accessory::{AccessoryListType,AccessoryProtocolListType,AccessoryListRequest};
/// AccessoryListRequest{_type: Some(AccessoryListType::Remote), protocol: Some(AccessoryProtocolListType::BluetoothLE)};
/// ```
/// List all RF4CE remotes
/// ```
/// use ripple_sdk::api::device::device_accessory::{AccessoryListType,AccessoryProtocolListType,AccessoryListRequest};
/// AccessoryListRequest{_type: Some(AccessoryListType::Remote), protocol: Some(AccessoryProtocolListType::RF4CE)};
/// ```
/// List all All devices
/// ```
/// use ripple_sdk::api::device::device_accessory::{AccessoryListType,AccessoryProtocolListType,AccessoryListRequest};
/// AccessoryListRequest{_type: Some(AccessoryListType::All), protocol: Some(AccessoryProtocolListType::All)};
/// ```
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AccessoryListRequest {
    #[serde(rename = "type")]
    pub _type: Option<AccessoryListType>,
    pub protocol: Option<AccessoryProtocolListType>,
}

impl Default for AccessoryListRequest {
    fn default() -> Self {
        AccessoryListRequest {
            _type: Some(AccessoryListType::All),
            protocol: Some(AccessoryProtocolListType::All),
        }
    }
}

/// Enumeration to enlist the different query params based on the connected protocol on a Accessory List request.
///
/// # More info
///
/// ```
/// use ripple_sdk::api::device::device_accessory::AccessoryProtocol;
/// AccessoryProtocol::BluetoothLE; // List of Devices connected via BluetoothLe
/// AccessoryProtocol::RF4CE; // List of Devices connected via RF4CE(Radio Frequency).
/// ```
///
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash, Eq)]
pub enum AccessoryProtocol {
    BluetoothLE,
    RF4CE,
}

impl AccessoryProtocol {
    pub fn get_supported_protocol(value: DeviceManifest) -> Self {
        let supported_caps = value.get_supported_caps();
        if supported_caps.contains(&FireboltCap::short("remote:rf4ce")) {
            AccessoryProtocol::RF4CE
        } else {
            AccessoryProtocol::BluetoothLE
        }
    }
}

/// Constructs a response for a paired device response.
///
/// # Examples
/// Response object for BluetoothLE Remote paired made by "Some company" with "Some model".
/// ```
/// use ripple_sdk::api::device::device_accessory::{AccessoryType,AccessoryProtocol,AccessoryDeviceResponse};
/// let response = AccessoryDeviceResponse{_type: AccessoryType::Remote, protocol: AccessoryProtocol::BluetoothLE , make: "Some Company".into(), model: "Some model".into()};
/// ```
///
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AccessoryDeviceResponse {
    #[serde(rename = "type")]
    pub _type: AccessoryType,
    pub make: String,
    pub model: String,
    pub protocol: AccessoryProtocol,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AccessoryDeviceListResponse {
    pub list: Vec<AccessoryDeviceResponse>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::manifest::device_manifest::tests::Mockable;
    use crate::utils::test_utils::test_extn_payload_provider;
    use rstest::rstest;

    #[rstest(
        supported_caps,
        expected_protocol,
        case(vec!["xrn:firebolt:capability:remote:rf4ce".to_string()], AccessoryProtocol::RF4CE),
        case(vec!["xrn:firebolt:capability:remote:ble".to_string()], AccessoryProtocol::BluetoothLE),
        case(Vec::new(), AccessoryProtocol::BluetoothLE)
    )]
    fn test_get_supported_protocol(
        supported_caps: Vec<String>,
        expected_protocol: AccessoryProtocol,
    ) {
        let mut dm = DeviceManifest::mock();
        dm.capabilities.supported = supported_caps;
        let result = AccessoryProtocol::get_supported_protocol(dm);
        assert_eq!(result, expected_protocol);
    }

    #[test]
    fn test_extn_payload_provider_for_remote_accessory_request() {
        let accessory_pair_request = AccessoryPairRequest {
            _type: AccessoryType::Remote,
            protocol: AccessoryProtocol::BluetoothLE,
            timeout: 5000,
        };

        let remote_accessory_request = RemoteAccessoryRequest::Pair(accessory_pair_request);

        let contract_type: RippleContract = RippleContract::RemoteAccessory;
        test_extn_payload_provider(remote_accessory_request, contract_type);
    }
}
