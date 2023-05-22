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

use crate::utils::error::RippleError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RippleContract {
    Internal,
    AccountSession,
    Governance,
    Discovery,
    Launcher,
    PinChallenge,
    JsonRpsee,
    Config,
    LifecycleManagement,
    Rpc,
    ExtnStatus,
    DeviceInfo,
    Wifi,
    WindowManager,
    Browser,
    Permissions,
    BridgeProtocol,
    Storage,
    RemoteAccessory,
    Keyboard,
    SessionToken,
    AppEvents,
    DeviceEvents,
    PowerStateEvent,
    VoiceGuidance,
    SecureStorage,
    Advertising,
    PrivacySettings,
    Metrics,
    Entitlements,
    MediaEvents,
    WatchHistory,
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

impl Into<String> for RippleContract {
    /// Mainly used for [ExtnMessage] passing between Extensions
    /// Use `as_clear_string` method for plain string for references
    fn into(self) -> String {
        serde_json::to_string(&self).unwrap()
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
        if r.is_err() {
            Err(RippleError::ParseError)
        } else {
            contracts_string = r.unwrap();
            let mut contracts = Vec::new();
            for contract_string in contracts_string {
                if let Ok(contract) = RippleContract::try_from(contract_string) {
                    contracts.push(contract)
                }
            }
            Ok(ContractFulfiller { contracts })
        }
    }
}

impl Into<String> for ContractFulfiller {
    fn into(self) -> String {
        let mut contracts: Vec<Value> = Vec::new();
        for contract in self.contracts {
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
        assert!(if let Ok(RippleContract::DeviceInfo) = result {
            true
        } else {
            false
        });
    }
}
