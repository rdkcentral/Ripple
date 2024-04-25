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
        context::RippleContextUpdateRequest,
        device::device_info_request::{DeviceInfoRequest, DeviceResponse, FirmwareInfo},
        firebolt::{fb_metrics::MetricsContext, fb_openrpc::FireboltSemanticVersion},
        storage_property::StorageProperty,
    },
    chrono::{DateTime, Utc},
    extn::extn_client_message::ExtnResponse,
    log::error,
    utils::error::RippleError,
};

use rand::Rng;

use crate::processor::storage::storage_manager::StorageManager;

use super::platform_state::PlatformState;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

#[derive(Debug, Clone, Default)]
pub struct MetricsState {
    pub start_time: DateTime<Utc>,
    pub context: Arc<RwLock<MetricsContext>>,
    operational_telemetry_listeners: Arc<RwLock<HashSet<String>>>,
}

impl MetricsState {
    fn send_context_update_request(platform_state: &PlatformState) {
        let extn_client = platform_state.get_client().get_extn_client();
        let metrics_context = platform_state.metrics.context.read().unwrap().clone();

        if let Err(e) = extn_client
            .request_transient(RippleContextUpdateRequest::MetricsContext(metrics_context))
        {
            error!(
                "Error sending context update: RippleContextUpdateRequest::MetricsContext: {:?}",
                e
            );
        }
    }

    pub fn get_context(&self) -> MetricsContext {
        self.context.read().unwrap().clone()
    }
    pub async fn initialize(state: &PlatformState) {
        let metrics_percentage = state
            .get_device_manifest()
            .configuration
            .metrics_logging_percentage;

        let random_number = rand::thread_rng().gen_range(1..101);
        let metrics_enabled = random_number <= metrics_percentage;

        debug!(
            "initialize: metrics_percentage={}, random_number={}, enabled={}",
            metrics_percentage, random_number, metrics_enabled
        );

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

        let os_info = match Self::get_os_info_from_firebolt(state).await {
            Ok(info) => info,
            Err(_) => FirmwareInfo {
                name: "no.os.name.set".into(),
                version: FireboltSemanticVersion::new(0, 0, 0, "no.os.ver.set".into()),
            },
        };

        debug!("got os_info={:?}", &os_info);

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

        let firmware = match state
            .get_client()
            .send_extn_request(DeviceInfoRequest::PlatformBuildInfo)
            .await
        {
            Ok(resp) => {
                if let Some(DeviceResponse::PlatformBuildInfo(info)) = resp.payload.extract() {
                    info.name
                } else {
                    String::default()
                }
            }
            Err(_) => String::default(),
        };

        {
            // Time to set them
            let mut context = state.metrics.context.write().unwrap();

            context.enabled = metrics_enabled;

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
            context.os_name = os_info.name;
            context.os_ver = os_info.version.readable;
            context.device_name = device_name;
            context.device_session_id = String::from(&state.device_session_id);
            context.firmware = firmware;
            context.ripple_version = state
                .version
                .clone()
                .unwrap_or(String::from(SEMVER_LIGHTWEIGHT));

            if let Some(t) = timezone {
                context.device_timezone = t;
            }
        }
        {
            Self::update_account_session(state).await;
        }

        Self::send_context_update_request(state);
    }

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

    pub async fn update_account_session(state: &PlatformState) {
        {
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
        Self::send_context_update_request(state);
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

    pub fn update_session_id(&self, platform_state: PlatformState, value: Option<String>) {
        let value = value.unwrap_or_default();
        {
            let mut context = self.context.write().unwrap();
            context.device_session_id = value;
        }
        Self::send_context_update_request(&platform_state);
    }
}
