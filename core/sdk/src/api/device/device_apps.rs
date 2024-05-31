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
    api::app_catalog::AppMetadata,
    api::session::AccountSession,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

use super::device_request::DeviceRequest;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AppsRequest {
    Init,
    GetInstalledApps(Option<String>),
    InstallApp(DeviceAppMetadata),
    UninstallApp(InstalledApp),
    GetFireboltPermissions(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct InstalledApp {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceAppMetadata {
    pub id: String,
    pub title: String,
    pub version: String,
    pub uri: String,
    pub data: Option<String>,
}

impl DeviceAppMetadata {
    pub fn new(
        id: String,
        title: String,
        version: String,
        uri: String,
        data: Option<String>,
    ) -> DeviceAppMetadata {
        DeviceAppMetadata {
            id,
            title,
            version,
            uri,
            data,
        }
    }
}

impl From<AppMetadata> for DeviceAppMetadata {
    fn from(value: AppMetadata) -> Self {
        DeviceAppMetadata {
            id: value.id,
            title: value.title,
            version: value.version,
            uri: value.uri,
            data: value.data,
        }
    }
}

impl ExtnPayloadProvider for AppsRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Apps(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Apps(d))) = payload {
            return Some(d);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::Apps
    }
}

pub enum AppCatalogPoll {
    CheckForUpdates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_payload_provider_for_apps_request_get_apps() {
        let get_apps_request = AppsRequest::GetInstalledApps(Some(String::from("filter_criteria")));

        let contract_type: RippleContract = RippleContract::Apps;
        test_extn_payload_provider(get_apps_request, contract_type);
    }
}
