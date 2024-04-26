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
        context::{ActivationStatus, RippleContextUpdateRequest},
        device::device_info_request::{DeviceInfoRequest, DeviceResponse, FirmwareInfo},
        distributor::distributor_privacy::PrivacySettingsData,
        firebolt::{
            fb_metrics::{MetricsContext, MetricsEnvironment},
            fb_openrpc::FireboltSemanticVersion,
        },
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

const ONTOLOGY_PERSISTENT_STORAGE_NAMESPACE: &str = "accountProfile";
const ONTOLOGY_PERSISTENT_STORAGE_KEY_PROPOSITION: &str = "proposition";
const ONTOLOGY_PERSISTENT_STORAGE_KEY_RETAILER: &str = "retailer";
const ONTOLOGY_PERSISTENT_STORAGE_KEY_JVAGENT: &str = "jvagent";
const ONTOLOGY_PERSISTENT_STORAGE_KEY_COAM: &str = "coam";
const ONTOLOGY_PERSISTENT_STORAGE_KEY_ACCOUNT_TYPE: &str = "accountType";
const ONTOLOGY_PERSISTENT_STORAGE_KEY_OPERATOR: &str = "operator";
const ONTOLOGY_PERSISTENT_STORAGE_ACCOUNT_DETAIL_TYPE: &str = "detailType";
const ONTOLOGY_PERSISTENT_STORAGE_ACCOUNT_DEVICE_TYPE: &str = "deviceType";
const ONTOLOGY_PERSISTENT_STORAGE_ACCOUNT_DEVICE_MANUFACTURER: &str = "deviceManufacturer";

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
    // <pca>
    fn update_cet_list(&self, privacy_settings_data: &PrivacySettingsData) {
        let mut cet_list = Vec::new();

        if let Some(v) = privacy_settings_data.allow_business_analytics {
            if v {
                cet_list.push(String::from("dataPlatform:cet:xvp:analytics:business"));
            }
        }

        if let Some(v) = privacy_settings_data.allow_resume_points {
            if v {
                cet_list.push(String::from(
                    "dataPlatform:cet:xvp:personalization:continueWatching",
                ));
            }
        }

        if let Some(v) = privacy_settings_data.allow_personalization {
            if v {
                cet_list.push(String::from(
                    "dataPlatform:cet:xvp:personalization:recommendation",
                ));
            }
        }

        if let Some(v) = privacy_settings_data.allow_product_analytics {
            if v {
                cet_list.push(String::from("dataPlatform:cet:xvp:analytics"));
            }
        }

        //let mut context = self.context.write().unwrap();
        self.context.write().unwrap().cet_list = if !cet_list.is_empty() {
            Some(cet_list)
        } else {
            None
        };
    }

    async fn get_persistent_store_string(
        state: &PlatformState,
        key: &'static str,
    ) -> Option<String> {
        match StorageManager::get_string_from_namespace(
            state,
            ONTOLOGY_PERSISTENT_STORAGE_NAMESPACE.to_string(),
            key,
        )
        .await
        {
            Ok(resp) => Some(resp.as_value()),
            Err(e) => {
                error!(
                    "get_persistent_store_string: Could not retrieve value: e={:?}",
                    e
                );
                None
            }
        }
    }

    async fn get_persistent_store_bool(state: &PlatformState, key: &'static str) -> Option<bool> {
        match StorageManager::get_bool_from_namespace(
            state,
            ONTOLOGY_PERSISTENT_STORAGE_NAMESPACE.to_string(),
            key,
        )
        .await
        {
            Ok(resp) => Some(resp.as_value()),
            Err(e) => {
                error!(
                    "get_persistent_store_bool: Could not retrieve value: e={:?}",
                    e
                );
                None
            }
        }
    }
    // </pca>
    pub fn get_privacy_settings_cache(&self) -> PrivacySettingsData {
        self.privacy_settings_cache.read().unwrap().clone()
    }
    pub fn update_privacy_settings_cache(&self, value: &PrivacySettingsData) {
        let mut cache = self.privacy_settings_cache.write().unwrap();
        *cache = value.clone();
        // <pca>
        self.update_cet_list(value);
        // </pca>
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

        // <pca>
        // let firmware = match state
        //     .get_client()
        //     .send_extn_request(DeviceInfoRequest::PlatformBuildInfo)
        //     .await
        // {
        //     Ok(resp) => {
        //         if let Some(DeviceResponse::PlatformBuildInfo(info)) = resp.payload.extract() {
        //             info.name
        //         } else {
        //             String::default()
        //         }
        //     }
        //     Err(_) => String::default(),
        // };
        let mut firmware = String::default();
        let mut env = None;

        match state
            .get_client()
            .send_extn_request(DeviceInfoRequest::PlatformBuildInfo)
            .await
        {
            Ok(resp) => {
                if let Some(DeviceResponse::PlatformBuildInfo(info)) = resp.payload.extract() {
                    firmware = info.name;
                    env = if info.debug {
                        Some(MetricsEnvironment::Dev.to_string())
                    } else {
                        Some(MetricsEnvironment::Prod.to_string())
                    };
                }
            }
            Err(_) => env = None,
        };

        let activated = match state.get_client().get_extn_client().get_activation_status() {
            Some(ActivationStatus::Activated) => Some(true),
            None => None,
            _ => Some(false),
        };

        // <pca> debug
        let proposition =
            Self::get_persistent_store_string(&state, ONTOLOGY_PERSISTENT_STORAGE_KEY_PROPOSITION)
                .await
                .unwrap_or("xsb".into());
        // </pca>

        let retailer =
            Self::get_persistent_store_string(&state, ONTOLOGY_PERSISTENT_STORAGE_KEY_RETAILER)
                .await;

        let jv_agent =
            Self::get_persistent_store_string(&state, ONTOLOGY_PERSISTENT_STORAGE_KEY_JVAGENT)
                .await;

        let platform = proposition.clone();

        let coam =
            Self::get_persistent_store_bool(&state, ONTOLOGY_PERSISTENT_STORAGE_KEY_COAM).await;

        let country = StorageManager::get_string(&state, StorageProperty::CountryCode)
            .await
            .ok();

        let region = StorageManager::get_string(&state, StorageProperty::Locality)
            .await
            .ok();

        // TODO: Not currently in PS, should it be?
        let account_type =
            Self::get_persistent_store_string(&state, ONTOLOGY_PERSISTENT_STORAGE_KEY_ACCOUNT_TYPE)
                .await;
        //let account_type = Some("SomeAccountType".into());

        let operator =
            Self::get_persistent_store_string(&state, ONTOLOGY_PERSISTENT_STORAGE_KEY_OPERATOR)
                .await;

        // TODO: Not currently in PS, should it be?
        let account_detail_type = Self::get_persistent_store_string(
            &state,
            ONTOLOGY_PERSISTENT_STORAGE_ACCOUNT_DETAIL_TYPE,
        )
        .await;
        //let account_detail_type = Some("SomeAccountDetailType".into());

        // TODO: Currently not used by ripple, needs to align with AS value somehow. Likely AS will
        // write to PS.
        let device_type = Self::get_persistent_store_string(
            &state,
            ONTOLOGY_PERSISTENT_STORAGE_ACCOUNT_DEVICE_TYPE,
        )
        .await
        .unwrap_or("SomeDeviceType".into());

        // TODO: Currently not used by ripple, needs to align with AS value somehow. AS team to check
        // the current source and possibly write to PS or let us know where they get it.
        let device_manufacturer = Self::get_persistent_store_string(
            &state,
            ONTOLOGY_PERSISTENT_STORAGE_ACCOUNT_DEVICE_MANUFACTURER,
        )
        .await
        .unwrap_or("SomeDeviceManufacturer".into());

        let authenticated = None;
        // </pca>

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
            // <pca>
            //context.device_name = device_name;
            context.device_name = Some(device_name);
            // </pca>
            context.device_session_id = String::from(&state.device_session_id);
            context.firmware = firmware;
            context.ripple_version = state
                .version
                .clone()
                .unwrap_or(String::from(SEMVER_LIGHTWEIGHT));

            if let Some(t) = timezone {
                context.device_timezone = t;
            }

            // <pca>
            context.env = env;
            context.activated = activated;
            context.proposition = proposition;
            context.retailer = retailer;
            context.jv_agent = jv_agent;
            context.platform = platform;
            context.coam = coam;
            context.country = country;
            context.region = region;
            context.account_type = account_type;
            context.operator = operator;
            context.account_detail_type = account_detail_type;
            context.device_type = device_type;
            context.device_manufacturer = device_manufacturer;
            context.authenticated = authenticated;
            // </pca>
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

    // <pca>
    // pub async fn update_account_session(state: &PlatformState) {
    //     {
    //         let mut context = state.metrics.context.write().unwrap();
    //         let account_session = state.session_state.get_account_session();
    //         if let Some(session) = account_session {
    //             context.account_id = session.account_id;
    //             context.device_id = session.device_id;
    //             context.distribution_tenant_id = session.id;
    //         } else {
    //             context.account_id = "no.account.set".to_string();
    //             context.device_id = "no.device_id.set".to_string();
    //             context.distribution_tenant_id = "no.distribution_tenant_id.set".to_string();
    //         }
    //     }
    //     Self::send_context_update_request(state);
    // }
    pub async fn update_account_session(state: &PlatformState) {
        {
            let mut context = state.metrics.context.write().unwrap();
            let account_session = state.session_state.get_account_session();
            if let Some(session) = account_session {
                context.account_id = Some(session.account_id);
                context.device_id = Some(session.device_id);
                context.distribution_tenant_id = session.id;
            } else {
                context.account_id = None;
                context.device_id = None;
                context.distribution_tenant_id = "no.distribution_tenant_id.set".to_string();
            }
        }
        Self::send_context_update_request(state);
    }
    // </pca>

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
