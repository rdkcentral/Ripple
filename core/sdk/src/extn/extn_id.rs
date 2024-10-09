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
    framework::ripple_contract::{ContractAdjective, RippleContract},
    utils::error::RippleError,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ExtnClassId {
    Gateway,
    Device,
    DataGovernance,
    Distributor,
    Protected,
    #[default]
    Jsonrpsee,
    Launcher,
    Internal,
}

impl ToString for ExtnClassId {
    fn to_string(&self) -> String {
        match self {
            Self::Gateway => "gateway".into(),
            Self::Device => "device".into(),
            Self::DataGovernance => "data-governance".into(),
            Self::Distributor => "distributor".into(),
            Self::Protected => "rpc".into(),
            Self::Jsonrpsee => "jsonrpsee".into(),
            Self::Launcher => "launcher".into(),
            Self::Internal => "internal".into(),
        }
    }
}

impl ExtnClassId {
    pub fn get(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "device" => Some(Self::Device),
            "data-governance" => Some(Self::DataGovernance),
            "distributor" => Some(Self::Distributor),
            "protected" => Some(Self::Protected),
            "jsonrpsee" => Some(Self::Jsonrpsee),
            "launcher" => Some(Self::Launcher),
            "internal" => Some(Self::Internal),
            "gateway" => Some(Self::Gateway),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ExtnType {
    #[default]
    Main,
    Channel,
    Extn,
}

impl ToString for ExtnType {
    fn to_string(&self) -> String {
        match self {
            Self::Main => "main".into(),
            Self::Channel => "channel".into(),
            Self::Extn => "extn".into(),
        }
    }
}

impl ExtnType {
    pub fn get(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "main" => Some(Self::Main),
            "channel" => Some(Self::Channel),
            "extn" => Some(Self::Extn),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtnClassType {
    _type: ExtnType,
    class: ExtnClassId,
}

impl ExtnClassType {
    pub fn new(_type: ExtnType, class: ExtnClassId) -> ExtnClassType {
        ExtnClassType { _type, class }
    }

    pub fn get_cap(&self, service: String) -> ExtnId {
        ExtnId {
            _type: self._type.clone(),
            class: self.class.clone(),
            service,
        }
    }
}

/// Below is anatomy of a ExtnId in a String format
///
/// `ripple:[channel/extn]:[class]:[service]:[feature (optional)]`

/// ## Decoding some ExtnCapabilit(ies)
///
/// Below capability means the given plugin offers a channel for Device Connection. Service used  for the device connection is Thunder.
///
/// `ripple:channel:device:thunder`
///
/// Below Capability means the given plugin offers a channel for Data Governance. Service used for the feature is `somegovernance` service
///
/// `ripple:channel:data-governance:somegovernance`
///
/// Below capability means the given plugin offers an extension for the Device Thunder Channel. It offers the auth thunder plugin implementation.
///
/// `ripple:extn:device:thunder:auth`
///
/// Below capability means the given plugin offers a permission extension for the distributor. Name of the service is called `fireboltpermissions`.
///
/// `ripple:extn:distributor:permissions:fireboltpermissions`
///
/// Below capability means the given plugin offers a JsonRpsee rpc extension for a service named bridge
///
/// `ripple:extn:jsonrpsee:bridge`
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExtnId {
    pub _type: ExtnType,
    pub class: ExtnClassId,
    pub service: String,
}

impl Serialize for ExtnId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ExtnId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if let Ok(str) = String::deserialize(deserializer) {
            if let Ok(id) = ExtnId::try_from(str) {
                return Ok(id);
            }
        }
        Err(serde::de::Error::unknown_variant("unknown", &["unknown"]))
    }
}

impl ToString for ExtnId {
    fn to_string(&self) -> String {
        let r = format!(
            "ripple:{}:{}:{}",
            self._type.to_string(),
            self.class.to_string(),
            self.service
        );
        r
    }
}

impl TryFrom<String> for ExtnId {
    type Error = RippleError;

    fn try_from(cap: String) -> Result<Self, Self::Error> {
        let c_a = cap.split(':').collect::<Vec<_>>();

        match c_a
            .first()
            .ok_or(RippleError::ParseError)?
            .to_lowercase()
            .as_str()
        {
            "ripple" => {}
            _ => return Err(RippleError::ParseError),
        }

        let _type = ExtnType::get(c_a.get(1).ok_or(RippleError::ParseError)?)
            .ok_or(RippleError::ParseError)?;
        let class = ExtnClassId::get(c_a.get(2).ok_or(RippleError::ParseError)?)
            .ok_or(RippleError::ParseError)?;
        let service = c_a.get(3).ok_or(RippleError::ParseError)?;

        Ok(ExtnId {
            _type,
            class,
            service: String::from(*service),
        })
    }
}

impl ExtnId {
    /// Returns the Main target capability for a given service
    /// # Arguments
    /// `service` - Type of String which defins the type of service
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_id::ExtnId;
    ///
    /// let main_cap = ExtnId::get_main_target("cap".into());
    /// assert!(main_cap.to_string().eq("ripple:main:internal:cap"));
    /// ```
    pub fn get_main_target(service: String) -> ExtnId {
        ExtnId {
            _type: ExtnType::Main,
            class: ExtnClassId::Internal,
            service,
        }
    }

    /// Checks if the given capability is a channel.
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_id::{ExtnId,ExtnClassId};
    ///
    /// let device_channel = ExtnId::new_channel(ExtnClassId::Device, "info".into());
    /// assert!(device_channel.is_channel());
    /// ```
    pub fn is_channel(&self) -> bool {
        if let ExtnType::Channel = self._type {
            return true;
        }
        false
    }

    /// Checks if the given capability is a extn.
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_id::{ExtnId,ExtnClassId};
    ///
    /// let device_channel = ExtnId::new_extn(ExtnClassId::Device, "info".into());
    /// assert!(device_channel.is_extn());
    /// ```
    pub fn is_extn(&self) -> bool {
        if let ExtnType::Extn = self._type {
            return true;
        }
        false
    }

    /// Checks if the given capability is a launcher channel.
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_id::{ExtnId,ExtnClassId};
    ///
    /// let launcher_channel = ExtnId::get_main_target("cap".into());
    /// assert!(launcher_channel.is_main());
    /// ```
    pub fn is_main(&self) -> bool {
        if let ExtnType::Main = self._type {
            return true;
        }
        false
    }

    /// Checks if the given capability is a device channel.
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_id::{ExtnId,ExtnClassId};
    ///
    /// let device_channel = ExtnId::new_channel(ExtnClassId::Device, "info".into());
    /// assert!(device_channel.is_device_channel());
    /// ```
    pub fn is_device_channel(&self) -> bool {
        if let ExtnType::Channel = self._type {
            if let ExtnClassId::Device = self.class {
                return true;
            }
        }
        false
    }

    /// Checks if the given capability is a launcher channel.
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_id::{ExtnId,ExtnClassId};
    ///
    /// let launcher_channel = ExtnId::new_channel(ExtnClassId::Launcher, "info".into());
    /// assert!(launcher_channel.is_launcher_channel());
    /// ```
    pub fn is_launcher_channel(&self) -> bool {
        if let ExtnType::Channel = self._type {
            if let ExtnClassId::Launcher = self.class {
                return true;
            }
        }
        false
    }

    /// Checks if the given capability is a distributor channel.
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_id::{ExtnId,ExtnClassId};
    ///
    /// let dist_channel = ExtnId::new_channel(ExtnClassId::Distributor, "general".into());
    /// assert!(dist_channel.is_distributor_channel());
    /// ```
    pub fn is_distributor_channel(&self) -> bool {
        if let ExtnType::Channel = self._type {
            if let ExtnClassId::Distributor = self.class {
                return true;
            }
        }
        false
    }

    /// Checks if the given capability has the same type and channel as the owner
    /// # Arguments
    /// `ref_cap` - Type of ExtnId which will be checked against the owner.
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_id::{ExtnId,ExtnClassId};
    ///
    /// let info = ExtnId::new_channel(ExtnClassId::Device, "info".into());
    /// let remote = ExtnId::new_channel(ExtnClassId::Device, "remote".into());
    /// assert!(info.match_layer(remote));
    /// ```
    pub fn match_layer(&self, ref_cap: ExtnId) -> bool {
        if ref_cap._type == ExtnType::Main && self._type == ExtnType::Main {
            return true;
        }

        if self._type == ExtnType::Channel
            && ref_cap._type == self._type
            && ref_cap.class == self.class
        {
            return true;
        }

        if self._type == ExtnType::Extn
            && ref_cap.class == self.class
            && ref_cap.service == self.service
        {
            return true;
        }

        false
    }

    /// Gets a short form value of the given capability. Useful for ExtnClient to detect
    /// other senders for a given ExtnId.
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_id::{ExtnId,ExtnClassId};
    ///
    /// let device_channel = ExtnId::new_channel(ExtnClassId::Device, "info".into());
    /// assert!(device_channel.get_short().eq("Channel:Device"));
    /// ```
    pub fn get_short(&self) -> String {
        if self._type == ExtnType::Channel {
            format!("{:?}:{:?}", self._type, self.class)
        } else {
            format!("{:?}:{}", self.class, self.service)
        }
    }

    /// Returns the `ExtnType` for the capability
    pub fn get_type(&self) -> ExtnType {
        self._type.clone()
    }

    /// Returns the `ExtnClass` for the capability
    pub fn class(&self) -> ExtnClassId {
        self.class.clone()
    }

    /// Gets a new Channel capability based on the given `ExtnClass` and `Service`
    /// # Arguments
    /// `class` - Type of [ExtnClass]
    /// `service` - Type of String which defines a unique service for a given Class channel
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_id::{ExtnId,ExtnClassId};
    ///
    /// let device_channel = ExtnId::new_channel(ExtnClassId::Device, "info".into());
    /// assert!(device_channel.get_short().eq("Channel:Device"));
    /// ```
    pub fn new_channel(class: ExtnClassId, service: String) -> Self {
        Self {
            _type: ExtnType::Channel,
            class,
            service,
        }
    }

    /// Gets a new Channel capability based on the given `ExtnClass` and `Service`
    /// # Arguments
    /// `class` - Type of [ExtnClass]
    /// `service` - Type of String which defines a unique service for a given Class channel
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_id::{ExtnId,ExtnClassId};
    ///
    /// let device_channel = ExtnId::new_extn(ExtnClassId::Device, "remote".into());
    /// assert!(device_channel.get_short().eq("Device:remote"));
    /// ```
    pub fn new_extn(class: ExtnClassId, service: String) -> Self {
        Self {
            _type: ExtnType::Extn,
            class,
            service,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtnProviderAdjective {
    pub id: ExtnId,
}

impl Serialize for ExtnProviderAdjective {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.id.to_string())
    }
}

impl<'de> Deserialize<'de> for ExtnProviderAdjective {
    fn deserialize<D>(deserializer: D) -> Result<ExtnProviderAdjective, D::Error>
    where
        D: Deserializer<'de>,
    {
        if let Ok(str) = String::deserialize(deserializer) {
            if let Ok(id) = ExtnId::try_from(str) {
                return Ok(ExtnProviderAdjective { id });
            }
        }
        Err(serde::de::Error::unknown_variant("unknown", &["unknown"]))
    }
}

impl ContractAdjective for ExtnProviderAdjective {
    fn get_contract(&self) -> RippleContract {
        RippleContract::ExtnProvider(self.clone())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtnProviderRequest {
    pub value: Value,
    pub id: ExtnId,
}

impl ExtnPayloadProvider for ExtnProviderRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Extn(value)) = payload {
            if let Ok(v) = serde_json::from_value::<ExtnProviderRequest>(value) {
                return Some(v);
            }
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Extn(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn contract() -> RippleContract {
        // Will be replaced by the IEC before CExtnMessage conversion
        RippleContract::ExtnProvider(ExtnProviderAdjective {
            id: ExtnId::get_main_target("default".into()),
        })
    }

    fn get_contract(&self) -> RippleContract {
        RippleContract::ExtnProvider(ExtnProviderAdjective {
            id: self.id.clone(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtnProviderResponse {
    pub value: Value,
}

impl ExtnPayloadProvider for ExtnProviderResponse {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::Value(value)) = payload {
            return Some(ExtnProviderResponse { value });
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(self.value.clone()))
    }

    fn contract() -> RippleContract {
        // Will be replaced by the IEC before CExtnMessage conversion
        RippleContract::ExtnProvider(ExtnProviderAdjective {
            id: ExtnId::get_main_target("default".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extn_class_id_to_string() {
        assert_eq!(ExtnClassId::Gateway.to_string(), "gateway");
        assert_eq!(ExtnClassId::Device.to_string(), "device");
        assert_eq!(ExtnClassId::DataGovernance.to_string(), "data-governance");
        assert_eq!(ExtnClassId::Distributor.to_string(), "distributor");
        assert_eq!(ExtnClassId::Protected.to_string(), "rpc");
        assert_eq!(ExtnClassId::Jsonrpsee.to_string(), "jsonrpsee");
        assert_eq!(ExtnClassId::Launcher.to_string(), "launcher");
        assert_eq!(ExtnClassId::Internal.to_string(), "internal");
    }

    #[test]
    fn test_extn_class_id_get() {
        assert_eq!(ExtnClassId::get("device"), Some(ExtnClassId::Device));
        assert_eq!(ExtnClassId::get("unknown"), None);
        // Add similar assertions for other variants
    }

    #[test]
    fn test_extn_type_to_string() {
        assert_eq!(ExtnType::Main.to_string(), "main");
        assert_eq!(ExtnType::Channel.to_string(), "channel");
        assert_eq!(ExtnType::Extn.to_string(), "extn");
    }

    #[test]
    fn test_extn_type_get() {
        assert_eq!(ExtnType::get("main"), Some(ExtnType::Main));
        assert_eq!(ExtnType::get("unknown"), None);
        // Add similar assertions for other variants
    }

    #[test]
    fn test_extn_class_type_new() {
        let extn_class_type = ExtnClassType::new(ExtnType::Main, ExtnClassId::Internal);
        assert_eq!(extn_class_type._type, ExtnType::Main);
        assert_eq!(extn_class_type.class, ExtnClassId::Internal);
    }

    #[test]
    fn test_extn_id_to_string() {
        let extn_id = ExtnId::new_channel(ExtnClassId::Device, "info".into());
        assert_eq!(extn_id.to_string(), "ripple:channel:device:info");
    }

    #[test]
    fn test_extn_id_try_from() {
        let extn_id_str = "ripple:channel:device:info";
        let extn_id = ExtnId::try_from(extn_id_str.to_string()).unwrap();
        assert_eq!(extn_id._type, ExtnType::Channel);
        assert_eq!(extn_id.class, ExtnClassId::Device);
        assert_eq!(extn_id.service, "info");
    }

    #[test]
    fn test_extn_id_get_main_target() {
        let main_cap = ExtnId::get_main_target("cap".into());
        assert_eq!(main_cap.to_string(), "ripple:main:internal:cap");
    }

    #[test]
    fn test_extn_id_is_channel() {
        let device_channel = ExtnId::new_channel(ExtnClassId::Device, "info".into());
        assert!(device_channel.is_channel());
    }

    #[test]
    fn test_extn_id_is_extn() {
        let device_channel = ExtnId::new_extn(ExtnClassId::Device, "info".into());
        assert!(device_channel.is_extn());
    }

    #[test]
    fn test_extn_id_is_main() {
        let launcher_channel = ExtnId::get_main_target("cap".into());
        assert!(launcher_channel.is_main());
    }

    #[test]
    fn test_extn_id_is_device_channel() {
        let device_channel = ExtnId::new_channel(ExtnClassId::Device, "info".into());
        assert!(device_channel.is_device_channel());
    }

    #[test]
    fn test_extn_id_is_launcher_channel() {
        let launcher_channel = ExtnId::new_channel(ExtnClassId::Launcher, "info".into());
        assert!(launcher_channel.is_launcher_channel());
    }

    #[test]
    fn test_extn_id_is_distributor_channel() {
        let dist_channel = ExtnId::new_channel(ExtnClassId::Distributor, "general".into());
        assert!(dist_channel.is_distributor_channel());
    }

    #[test]
    fn test_extn_id_match_layer() {
        let info = ExtnId::new_channel(ExtnClassId::Device, "info".into());
        let remote = ExtnId::new_channel(ExtnClassId::Device, "remote".into());
        assert!(info.match_layer(remote));
    }

    #[test]
    fn test_extn_id_get_short() {
        let device_channel = ExtnId::new_channel(ExtnClassId::Device, "info".into());
        assert_eq!(device_channel.get_short(), "Channel:Device");
    }

    #[test]
    fn test_extn_class_id_get_unknown() {
        assert_eq!(ExtnClassId::get("unknown"), None);
        // Add similar assertions for other variants
    }

    #[test]
    fn test_extn_type_get_unknown() {
        assert_eq!(ExtnType::get("unknown"), None);
        // Add similar assertions for other variants
    }

    #[test]
    fn test_extn_class_type_get_cap() {
        let extn_class_type = ExtnClassType::new(ExtnType::Main, ExtnClassId::Internal);
        let extn_id = extn_class_type.get_cap("some_service".into());
        assert_eq!(extn_id.to_string(), "ripple:main:internal:some_service");
    }

    #[test]
    fn test_extn_id_try_from_invalid_format() {
        let extn_id_str = "invalid_format";
        assert!(ExtnId::try_from(extn_id_str.to_string()).is_err());
    }

    #[test]
    fn test_extn_id_try_from_incomplete_fields() {
        let extn_id_str = "ripple:channel:device";
        assert!(ExtnId::try_from(extn_id_str.to_string()).is_err());
    }

    #[test]
    fn test_extn_id_try_from_invalid_prefix() {
        let extn_id_str = "invalid:channel:device:info";
        assert!(ExtnId::try_from(extn_id_str.to_string()).is_err());
    }

    #[test]
    fn test_extn_id_try_from_valid() {
        let extn_id_str = "ripple:channel:device:info";
        let extn_id = ExtnId::try_from(extn_id_str.to_string()).unwrap();
        assert_eq!(extn_id._type, ExtnType::Channel);
        assert_eq!(extn_id.class, ExtnClassId::Device);
        assert_eq!(extn_id.service, "info");
    }

    #[test]
    fn test_extn_id_eq() {
        let extn_id1 = ExtnId::new_channel(ExtnClassId::Device, "info".into());
        let extn_id2 = ExtnId::new_channel(ExtnClassId::Device, "info".into());
        assert_eq!(extn_id1, extn_id2);
    }
}
