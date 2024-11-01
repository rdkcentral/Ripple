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
    api::storage_property::StorageAdjective,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::device_request::DeviceRequest;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct StorageData {
    pub value: Value,
    pub update_time: String, // ISO 8601/RFC3339 format
}

impl StorageData {
    pub fn new(value: Value) -> StorageData {
        StorageData {
            value,
            update_time: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct SetPropertyOpt<T> {
    pub value: Option<T>,
}

#[derive(Deserialize, Debug)]
pub struct SetProperty<T> {
    pub value: T,
}

#[derive(Deserialize, Debug)]
pub struct SetBoolProperty {
    pub value: bool,
}

#[derive(Deserialize, Debug)]
pub struct SetStringProperty {
    pub value: String,
}

#[derive(Deserialize, Debug)]
pub struct SetU32Property {
    pub value: u32,
}

#[derive(Deserialize, Debug)]
pub struct SetF32Property {
    pub value: f32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct GetStorageProperty {
    pub namespace: String,
    pub key: String,
    pub scope: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SetStorageProperty {
    pub namespace: String,
    pub key: String,
    pub data: StorageData,
    pub scope: Option<String>,
}

pub type DeleteStorageProperty = GetStorageProperty;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum DevicePersistenceRequest {
    Get(GetStorageProperty),
    Set(SetStorageProperty),
    Delete(DeleteStorageProperty),
}

impl ExtnPayloadProvider for DevicePersistenceRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Storage(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Storage(d))) = payload {
            return Some(d);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Storage(StorageAdjective::Local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_payload_provider_for_device_persistence_request_get() {
        let get_request = GetStorageProperty {
            namespace: String::from("example_namespace"),
            key: String::from("example_key"),
            scope: None,
        };

        let device_persistence_get_request = DevicePersistenceRequest::Get(get_request);

        let contract_type: RippleContract = RippleContract::Storage(StorageAdjective::Local);
        test_extn_payload_provider(device_persistence_get_request, contract_type);
    }
}
