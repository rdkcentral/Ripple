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

use crate::{
    api::{
        session::{EventAdjective, PubSubAdjective, SessionAdjective},
        storage_property::StorageAdjective,
    },
    extn::extn_id::ExtnProviderAdjective,
    utils::{error::RippleError, serde_utils::SerdeClearString},
};
use jsonrpsee_core::DeserializeOwned;
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Ripple Contract is the building block of Ripple Extension ecosystem.
/// A concrete unit of work expected to be available through extensions.
/// These contracts are not bound to a particular ExtnClass or ExtnType. Depending on a distributor implementation this contract can be fulfilled from
/// a. Device Extn
/// b. Distributor Extn/Channel
/// c. Combination of a Device + Distributor Extensions

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RippleContract {
    /// Used by Main application to provide internal contracts for Extensions
    Internal,
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
    /// Contract for supporting Wifi operations usually needed for settings
    Wifi,
    /// Denotes launch and manage browsers capabilities, used by launcher extension would become an adjective in
    /// the near future
    WindowManager,
    /// Provides options for handling Browser in terms of forwarding events and setting properties.
    Browser,
    /// Provides list of permitted capabilities for a given application.
    Permissions,
    /// Alternate protocol mechanism to connect to Ripple.
    BridgeProtocol,
    /// Allows pairing and configuring hand-held remotes. Would soon become an Adjective for accessory.
    RemoteAccessory,
    /// Provides options for triggering the Keyboard provider UI.
    Keyboard,
    /// Forwarder for extensions to pass on Events back to the registered Applications through Main
    AppEvents,
    /// Device channel specific events which get cascaded across Main and Extensions like Power, HDCP
    DeviceEvents(EventAdjective),
    /// Contract for controlling Voice guidance typically offered by the Device Channel or the browser.
    VoiceGuidance,
    /// Distributor Contract for handling Advertising requirements.
    Advertising,
    /// Contract focussed on Aggregating the App Behavior metrics before sending to the Distributor ingestors.
    BehaviorMetrics,
    /// Contract focussed on getting more real time media playback events like Pause, Play, Seek useful for
    /// features like Continue Watching
    MediaEvents,
    /// Contract which controls User Privacy Settings will become an Adjective in near future
    PrivacySettings,
    /// Contract used for tracking Sign in / Sign Out across apps so Distributor can provide better discovery
    /// of the signed in Application.
    AccountLink,
    /// Contract to allow Extensions to  get and set Settings.
    Settings,
    /// Bi directional channel between the device and external service can be implemented by Distributors.
    /// Distributors can send unique topics on messages to control Privacy, Usergrants and Automation
    PubSub(PubSubAdjective),
    /// Used for synchronization enforcement between cloud and local data
    CloudSync,
    /// Extensions can use this contract to get more information on the firebolt capabilities  
    Caps,
    /// Distributors can add their encoding algorithms to account and device id for security.
    Encoder,
    /// Contract for Main to forward behavior and operational metrics to processors
    Metrics,
    /// Contract for Extensions to recieve Telemetry events from Main
    OperationalMetricListener,
    Observability,
    TelemetryEventsListener,
    Storage(StorageAdjective),
    /// Provided by the distributor could be a device extension or a cloud extension.
    /// Distributor gets the ability to configure and customize the generation of
    /// the Session information based on their policies. Used by [crate::api::session::AccountSession]
    Session(SessionAdjective),
    RippleContext,
    ExtnProvider(ExtnProviderAdjective),
    AppCatalog,
    Apps,
    // Runtime ability for a given distributor to turn off a certian feature
    RemoteFeatureControl,
}

pub trait ContractAdjective: serde::ser::Serialize + DeserializeOwned {
    fn as_string(&self) -> String {
        let adjective = SerdeClearString::as_clear_string(self);
        if let Some(contract) = self.get_contract().get_adjective_contract() {
            return format!("{}.{}", adjective, contract);
        }
        adjective
    }
    fn get_contract(&self) -> RippleContract;
}

impl TryFrom<String> for RippleContract {
    type Error = RippleError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let is_adjective = Self::is_adjective(&value);
        if is_adjective {
            let mut split = value.split('.');
            let adjective = split.next().unwrap();
            let common_contract = split.next().unwrap();
            if let Some(c) = RippleContract::from_adjective_string(common_contract, adjective) {
                return Ok(c);
            }
        }
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
    pub fn as_clear_string(&self) -> String {
        let contract = SerdeClearString::as_clear_string(self);
        if let Some(adjective) = self.get_adjective() {
            adjective
        } else {
            contract
        }
    }

    pub fn get_adjective(&self) -> Option<String> {
        match self {
            Self::Storage(adj) => Some(adj.as_string()),
            Self::Session(adj) => Some(adj.as_string()),
            Self::PubSub(adj) => Some(adj.as_string()),
            Self::DeviceEvents(adj) => Some(adj.as_string()),
            Self::ExtnProvider(adj) => Some(adj.id.to_string()),
            _ => None,
        }
    }

    fn get_contract_from_adjective<T: ContractAdjective>(str: &str) -> Option<RippleContract> {
        match serde_json::from_str::<T>(str) {
            Ok(v) => Some(v.get_contract()),
            Err(e) => {
                error!("contract parser_error={:?}", e);
                None
            }
        }
    }

    pub fn from_adjective_string(contract: &str, adjective: &str) -> Option<Self> {
        let adjective = format!("\"{}\"", adjective);
        match contract {
            "extn_provider" => {
                return Self::get_contract_from_adjective::<ExtnProviderAdjective>(&adjective)
            }
            "storage" => match serde_json::from_str::<StorageAdjective>(&adjective) {
                Ok(v) => return Some(v.get_contract()),
                Err(e) => error!("contract parser_error={:?}", e),
            },
            "session" => match serde_json::from_str::<SessionAdjective>(&adjective) {
                Ok(v) => return Some(v.get_contract()),
                Err(e) => error!("contract parser_error={:?}", e),
            },
            "pubsub" => match serde_json::from_str::<PubSubAdjective>(&adjective) {
                Ok(v) => return Some(v.get_contract()),
                Err(e) => error!("contract parser_error={:?}", e),
            },
            "device_events" => match serde_json::from_str::<EventAdjective>(&adjective) {
                Ok(v) => return Some(v.get_contract()),
                Err(e) => error!("contract parser_error={:?}", e),
            },
            _ => {}
        }
        None
    }

    pub fn get_adjective_contract(&self) -> Option<String> {
        match self {
            Self::Storage(_) => Some("storage".to_owned()),
            Self::Session(_) => Some("session".to_owned()),
            Self::ExtnProvider(_) => Some("extn_provider".to_owned()),
            Self::PubSub(_) => Some("pubsub".to_owned()),
            Self::DeviceEvents(_) => Some("device_events".to_owned()),
            _ => None,
        }
    }

    pub fn is_adjective(contract: &str) -> bool {
        contract.split('.').count() == 2
    }

    pub fn from_manifest(contract: &str) -> Option<Self> {
        // first check if its adjective
        if Self::is_adjective(contract) {
            if let Ok(v) = Self::try_from(contract.to_owned()) {
                return Some(v);
            }
        } else if let Ok(v) = Self::try_from(SerdeClearString::prep_clear_string(contract)) {
            return Some(v);
        }
        None
    }

    pub fn is_extn_provider(&self) -> Option<String> {
        if let RippleContract::ExtnProvider(e) = self {
            Some(e.id.to_string())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ContractFulfiller {
    pub contracts: Vec<RippleContract>,
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
    use crate::{
        api::storage_property::StorageAdjective, framework::ripple_contract::RippleContract,
    };

    #[test]
    fn test_into() {
        let value: String = RippleContract::DeviceInfo.into();
        assert!(value.eq("\"device_info\""));
        let result = RippleContract::try_from(value);
        assert!(result.is_ok());
        assert!(matches!(result, Ok(RippleContract::DeviceInfo)));
    }

    #[test]
    fn test_adjectives_try_from_serialized() {
        let value: String = RippleContract::Storage(StorageAdjective::Local).into();
        assert!(value.eq("{\"storage\":\"local\"}"));
        let result = RippleContract::try_from(value);
        assert!(result.is_ok());
        assert!(matches!(
            result,
            Ok(RippleContract::Storage(StorageAdjective::Local))
        ));
    }

    #[test]
    fn test_adjectives_try_from_manifested() {
        // other way around
        let manifest_entry = String::from("local.storage");
        let result = RippleContract::try_from(manifest_entry);
        assert!(result.is_ok());
        assert!(matches!(
            result,
            Ok(RippleContract::Storage(StorageAdjective::Local))
        ));
    }

    #[test]
    fn test_as_clear_string() {
        assert_eq!(
            RippleContract::AccountLink.as_clear_string(),
            "account_link".to_owned()
        );
        assert_eq!(
            RippleContract::Session(crate::api::session::SessionAdjective::Account)
                .as_clear_string(),
            "account.session".to_owned()
        )
    }

    #[test]
    fn test_from_manifest() {
        assert!(RippleContract::from_manifest("account_link").is_some());
        assert!(RippleContract::from_manifest("account.session").is_some());
    }
}
