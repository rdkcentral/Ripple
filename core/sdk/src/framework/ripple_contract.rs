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

use crate::utils::error::RippleError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Ripple Contract is the building block of Ripple Extension ecosystem.
/// A concrete unit of work expected to be available through extensions.
/// These contracts are not bound to a particular ExtnClass or ExtnType. Depending on a distributor implementation this contract can be fulfilled from
/// a. Device Extn
/// b. Distributor Extn/Channel
/// c. Combination of a Device + Distributor Extensions

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RippleContract {
    /// Used by Main application to provide internal contracts for Extensions
    Internal,
    /// Provided by the distributor could be a device extension or a cloud extension.
    /// Distributor gets the ability to configure and customize the generation of
    /// the Session information based on their policies. Used by [crate::api::session::AccountSession]
    AccountSession,
    /// Provided by the distributor useful for adding Governance implementation for handling
    /// privacy information and other sensitive data.
    Governance,
    /// Provided by the distributor to discover content, apps, history and recommendations.
    /// Used by [crate::api::distributor::distributor_discovery::DiscoveryRequest]
    Discovery,
    /// Provided by the platform to handle launching and managing applications.
    /// Used by [crate::api::firebolt::fb_lifecycle_management::LifecycleManagementEventRequest]
    Launcher,
    /// Provided by the platform to support Pin challenge request from extensions. Used by [crate::api::firebolt::fb_pin::PinChallengeRequest]
    PinChallenge,
    /// Provided by the distributor for any additional RPC extensions doesnt use a request object.
    /// It is loaded by Extension manager during startup
    JsonRpsee,
    /// Provided by the platform as part of the Main application.
    /// Used by [crate::api::config::Config]
    Config,
    /// Provided by the Main application to help launcher application to get session and state.
    /// Used by [crate::api::firebolt::fb_lifecycle_management::LifecycleManagementRequest]
    LifecycleManagement,
    /// Not Used right now reserved for non JsonRPsee methods
    Rpc,
    /// Provided by the platform to maintain status of the loaded extension.
    /// Used as a Event contract doesnt map to a request.
    ExtnStatus,
    /// Provided by the device channel extensino for information specific to the device.
    /// Used by [crate::api::device::device_info_request::DeviceInfoRequest]
    DeviceInfo,
    Wifi,
    WindowManager,
    Browser,
    Permissions,
    BridgeProtocol,
    DevicePersistence,
    RemoteAccessory,
    Keyboard,
    SessionToken,
    AppEvents,
    DeviceEvents,
    PowerStateEvent,
    VoiceGuidance,
    SecureStorage,
    Advertising,
    PrivacyCloudStore,
    UserGrantsCloudStore,
    BehaviorMetrics,
    MediaEvents,
    PrivacySettings,
    StorageManager,
    AccountLink,
    Settings,
    PubSub,
    CloudSync,
    UserGrantsLocalStore,
    PrivacySettingsLocalStore,
    Caps,
    Encoder,
}

impl TryFrom<String> for RippleContract {
    type Error = RippleError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Ok(v) = serde_json::from_str(&value) {
            Ok(v)
        } else {
            Err(RippleError::ParseError)
        }
    }
}

impl From<RippleContract> for String {
    /// Mainly used for [ExtnMessage] passing between Extensions
    /// Use `as_clear_string` method for plain string for references
    fn from(val: RippleContract) -> Self {
        serde_json::to_string(&val).unwrap()
    }
}

impl RippleContract {
    /// This method gets the clear string of the contract without the quotes added
    /// by serde deserializer.
    pub fn as_clear_string(self) -> String {
        let s: String = self.into();
        s[1..s.len() - 1].into()
    }
}

#[derive(Debug, Clone)]
pub struct ContractFulfiller {
    contracts: Vec<RippleContract>,
}

impl ContractFulfiller {
    pub fn new(contracts: Vec<RippleContract>) -> ContractFulfiller {
        ContractFulfiller { contracts }
    }
}

impl TryFrom<String> for ContractFulfiller {
    type Error = RippleError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let r = serde_json::from_str(&value);
        let contracts_string: Vec<String>;
        if let Ok(r) = r {
            contracts_string = r;
            let mut contracts = Vec::new();
            for contract_string in contracts_string {
                if let Ok(contract) = RippleContract::try_from(contract_string) {
                    contracts.push(contract)
                }
            }
            Ok(ContractFulfiller { contracts })
        } else {
            Err(RippleError::ParseError)
        }
    }
}

impl From<ContractFulfiller> for String {
    fn from(val: ContractFulfiller) -> Self {
        let mut contracts: Vec<Value> = Vec::new();
        for contract in val.contracts {
            contracts.push(Value::String(contract.into()));
        }
        Value::Array(contracts).to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::framework::ripple_contract::RippleContract;

    #[test]
    fn test_into() {
        let value: String = RippleContract::DeviceInfo.into();
        assert!(value.eq("\"device_info\""));
        let result = RippleContract::try_from(value);
        assert!(result.is_ok());
        assert!(matches!(result, Ok(RippleContract::DeviceInfo)));
    }
}
