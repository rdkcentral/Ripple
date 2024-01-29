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

use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

use jsonrpsee::tracing::debug;
use ripple_sdk::{
    api::{
        device::device_info_request::{DeviceInfoRequest, DeviceResponse, FirmwareInfo},
        distributor::distributor_privacy::PrivacySettingsData,
        firebolt::{fb_metrics::MetricsContext, fb_openrpc::FireboltSemanticVersion},
        storage_property::StorageProperty,
    },
    chrono::{DateTime, Utc},
    extn::extn_client_message::ExtnResponse,
    utils::error::RippleError,
};

use crate::processor::storage::storage_manager::StorageManager;

use super::platform_state::PlatformState;

#[derive(Debug, Clone, Default)]
pub struct MetricsState {
    pub start_time: DateTime<Utc>,
    pub context: Arc<RwLock<MetricsContext>>,
    pub privacy_settings_cache: Arc<RwLock<PrivacySettingsData>>,
    operational_telemetry_listeners: Arc<RwLock<HashSet<String>>>,
}

impl MetricsState {
    pub fn get_context(&self) -> MetricsContext {
        self.context.read().unwrap().clone()
    }
    pub fn get_privacy_settings_cache(&self) -> PrivacySettingsData {
        self.privacy_settings_cache.read().unwrap().clone()
    }
    pub fn update_privacy_settings_cache(&self, value: &PrivacySettingsData) {
        let mut cache = self.privacy_settings_cache.write().unwrap();
        *cache = value.clone();
    }
    pub async fn initialize(state: &PlatformState) {
        let mut mac_address: Option<String> = None;
        if let Ok(resp) = state
            .get_client()
            .send_extn_request(DeviceInfoRequest::MacAddress)
            .await
        {
            if let Some(ExtnResponse::String(mac)) = resp.payload.extract() {
                let _ = mac_address.insert(mac);
            }
        }

        let mut serial_number: Option<String> = None;
        if let Ok(resp) = state
            .get_client()
            .send_extn_request(DeviceInfoRequest::SerialNumber)
            .await
        {
            if let Some(ExtnResponse::String(sn)) = resp.payload.extract() {
                let _ = serial_number.insert(sn);
            }
        }

        let mut device_model: Option<String> = None;
        if let Ok(resp) = state
            .get_client()
            .send_extn_request(DeviceInfoRequest::Model)
            .await
        {
            if let Some(ExtnResponse::String(model)) = resp.payload.extract() {
                let _ = device_model.insert(model);
            }
        }

        let language = match StorageManager::get_string(state, StorageProperty::Language).await {
            Ok(resp) => resp,
            Err(_) => "no.language.set".to_string(),
        };

        // <pca>
        // let os_ver = Self::get_os_ver_from_firebolt(state).await;
        // debug!("got os_ver={}", &os_ver);
        let os_info = match Self::get_os_info_from_firebolt(state).await {
            Ok(info) => info,
            Err(_) => FirmwareInfo {
                name: "no.os.name.set".into(),
                version: FireboltSemanticVersion::new(0, 0, 0, "no.os.ver.set".into()),
            },
        };
        debug!("got os_info={:?}", &os_info);
        // </pca>

        let mut device_name = "no.device.name.set".to_string();
        if let Ok(resp) = StorageManager::get_string(state, StorageProperty::DeviceName).await {
            device_name = resp;
        }

        let mut timezone: Option<String> = None;
        if let Ok(resp) = state
            .get_client()
            .send_extn_request(DeviceInfoRequest::GetTimezoneWithOffset)
            .await
        {
            if let Some(ExtnResponse::TimezoneWithOffset(tz, offset)) = resp.payload.extract() {
                timezone = Some(format!("{} {}", tz, offset));
            }
        }
        {
            // Time to set them
            let mut context = state.metrics.context.write().unwrap();
            if let Some(mac) = mac_address {
                context.mac_address = mac;
            }

            if let Some(sn) = serial_number {
                context.serial_number = sn;
            }

            if let Some(model) = device_model {
                context.device_model = model;
            }

            context.device_language = language;
            // <pca>
            //context.os_ver = os_ver;
            context.os_name = os_info.name;
            context.os_ver = os_info.version.readable;
            // </pca>
            context.device_name = device_name;
            context.device_session_id = String::from(&state.device_session_id);

            if let Some(t) = timezone {
                context.device_timezone = t;
            }
        }
        Self::update_account_session(state).await
    }

    // <pca>
    // async fn get_os_ver_from_firebolt(platform_state: &PlatformState) -> String {
    //     let mut os = FireboltSemanticVersion::new(0, 0, 0, "".to_string());

    //     if let Ok(val) = platform_state
    //         .get_client()
    //         .send_extn_request(DeviceInfoRequest::Version)
    //         .await
    //     {
    //         if let Some(DeviceResponse::FirmwareInfo(value)) = val.payload.extract() {
    //             os = value;
    //         }
    //     }
    //     let os_ver: String = if !os.readable.is_empty() {
    //         os.readable.to_string()
    //     } else {
    //         "no.os.ver.set".to_string()
    //     };
    //     os_ver
    // }
    async fn get_os_info_from_firebolt(
        platform_state: &PlatformState,
    ) -> Result<FirmwareInfo, RippleError> {
        match platform_state
            .get_client()
            .send_extn_request(DeviceInfoRequest::FirmwareInfo)
            .await
        {
            Ok(message) => {
                if let Some(DeviceResponse::FirmwareInfo(info)) = message.payload.extract() {
                    Ok(info)
                } else {
                    Err(RippleError::InvalidOutput)
                }
            }
            Err(e) => Err(e),
        }
    }
    // </pca>

    pub async fn update_account_session(state: &PlatformState) {
        let mut context = state.metrics.context.write().unwrap();
        let account_session = state.session_state.get_account_session();
        if let Some(session) = account_session {
            context.account_id = session.account_id;
            context.device_id = session.device_id;
            context.distribution_tenant_id = session.id;
        } else {
            context.account_id = "no.account.set".to_string();
            context.device_id = "no.device_id.set".to_string();
            context.distribution_tenant_id = "no.distribution_tenant_id.set".to_string();
        }
    }

    pub fn operational_telemetry_listener(&self, target: &str, listen: bool) {
        let mut listeners = self.operational_telemetry_listeners.write().unwrap();
        if listen {
            listeners.insert(target.to_string());
        } else {
            listeners.remove(target);
        }
    }

    pub fn get_listeners(&self) -> Vec<String> {
        self.operational_telemetry_listeners
            .read()
            .unwrap()
            .iter()
            .map(|x| x.to_owned())
            .collect()
    }

    pub fn update_session_id(&self, value: Option<String>) {
        let value = value.unwrap_or_default();
        let mut context = self.context.write().unwrap();
        context.device_session_id = value;
    }
}
