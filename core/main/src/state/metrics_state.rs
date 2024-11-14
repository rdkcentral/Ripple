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

use crate::broker::broker_utils::BrokerUtils;
use jsonrpsee::{tracing::debug, types::error::CallError};
use ripple_sdk::{
    api::{
        context::RippleContextUpdateRequest,
        device::device_info_request::{DeviceInfoRequest, DeviceResponse, FirmwareInfo},
        distributor::distributor_privacy::{DataEventType, PrivacySettingsData},
        firebolt::{
            fb_metrics::{MetricsContext, MetricsEnvironment},
            fb_openrpc::FireboltSemanticVersion,
        },
        manifest::device_manifest::DataGovernanceConfig,
        storage_property::StorageProperty,
    },
    chrono::{DateTime, Utc},
    extn::extn_client_message::ExtnResponse,
    log::error,
    utils::error::RippleError,
};
use serde_json::from_value;

use rand::Rng;

use crate::processor::storage::storage_manager::StorageManager;

use super::platform_state::PlatformState;

include!(concat!(env!("OUT_DIR"), "/version.rs"));

const PERSISTENT_STORAGE_NAMESPACE: &str = "accountProfile";
const PERSISTENT_STORAGE_KEY_PROPOSITION: &str = "proposition";
const PERSISTENT_STORAGE_KEY_RETAILER: &str = "retailer";
const PERSISTENT_STORAGE_KEY_PRIMARY_PROVIDER: &str = "jvagent";
const PERSISTENT_STORAGE_KEY_COAM: &str = "coam";
const PERSISTENT_STORAGE_KEY_ACCOUNT_TYPE: &str = "accountType";
const PERSISTENT_STORAGE_KEY_OPERATOR: &str = "operator";
const PERSISTENT_STORAGE_ACCOUNT_DETAIL_TYPE: &str = "detailType";
const PERSISTENT_STORAGE_ACCOUNT_DEVICE_TYPE: &str = "deviceType";
const PERSISTENT_STORAGE_ACCOUNT_DEVICE_MANUFACTURER: &str = "deviceManufacturer";

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

    fn get_option_string(s: String) -> Option<String> {
        if !s.is_empty() {
            return Some(s);
        }
        None
    }

    pub fn update_data_governance_tags(
        &self,
        platform_state: &PlatformState,
        privacy_settings_data: &PrivacySettingsData,
    ) {
        fn update_tags(
            data_governance_config: &DataGovernanceConfig,
            data: Option<bool>,
            tags: &mut Vec<String>,
            data_event_type: DataEventType,
            storage_property: StorageProperty,
        ) {
            if let Some(true) = data {
                if let Some(policy) = data_governance_config.get_policy(data_event_type) {
                    if let Some(setting_tag) = policy
                        .setting_tags
                        .iter()
                        .find(|t| t.setting == storage_property)
                    {
                        for tag in setting_tag.tags.clone() {
                            tags.push(tag);
                        }
                    }
                }
            }
        }

        let mut governance_tags: Vec<String> = Vec::new();
        let data_governance_config = platform_state
            .get_device_manifest()
            .configuration
            .data_governance;

        update_tags(
            &data_governance_config,
            privacy_settings_data.allow_business_analytics,
            &mut governance_tags,
            DataEventType::BusinessIntelligence,
            StorageProperty::AllowBusinessAnalytics,
        );

        update_tags(
            &data_governance_config,
            privacy_settings_data.allow_resume_points,
            &mut governance_tags,
            DataEventType::Watched,
            StorageProperty::AllowWatchHistory,
        );

        update_tags(
            &data_governance_config,
            privacy_settings_data.allow_personalization,
            &mut governance_tags,
            DataEventType::BusinessIntelligence,
            StorageProperty::AllowPersonalization,
        );

        update_tags(
            &data_governance_config,
            privacy_settings_data.allow_product_analytics,
            &mut governance_tags,
            DataEventType::BusinessIntelligence,
            StorageProperty::AllowProductAnalytics,
        );

        self.context.write().unwrap().data_governance_tags = if !governance_tags.is_empty() {
            Some(governance_tags)
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
            PERSISTENT_STORAGE_NAMESPACE.to_string(),
            key,
            None,
        )
        .await
        {
            Ok(resp) => Self::get_option_string(resp.as_value()),
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
            PERSISTENT_STORAGE_NAMESPACE.to_string(),
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

        let language = BrokerUtils::process_internal_main_request(state, "localization.language")
            .await
            .and_then(|val| {
                from_value::<String>(val).map_err(|_| {
                    jsonrpsee::core::Error::Call(CallError::Custom {
                        code: -32100,
                        message: "Failed to parse language".into(),
                        data: None,
                    })
                })
            })
            .unwrap_or_else(|_| "no.language.set".to_string());

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

        let activated = Some(true);
        let proposition =
            Self::get_persistent_store_string(state, PERSISTENT_STORAGE_KEY_PROPOSITION)
                .await
                .unwrap_or("Proposition.missing.from.persistent.store".into());

        let retailer =
            Self::get_persistent_store_string(state, PERSISTENT_STORAGE_KEY_RETAILER).await;

        let primary_provider =
            Self::get_persistent_store_string(state, PERSISTENT_STORAGE_KEY_PRIMARY_PROVIDER).await;

        let platform = proposition.clone();

        let coam = Self::get_persistent_store_bool(state, PERSISTENT_STORAGE_KEY_COAM).await;

        let country = BrokerUtils::process_internal_main_request(state, "localization.countryCode")
            .await
            .ok()
            .and_then(|val| from_value::<String>(val).ok());
        debug!("got country_code={:?}", &country);

        let region = StorageManager::get_string(state, StorageProperty::Locality)
            .await
            .ok();

        let account_type =
            Self::get_persistent_store_string(state, PERSISTENT_STORAGE_KEY_ACCOUNT_TYPE).await;

        let operator =
            Self::get_persistent_store_string(state, PERSISTENT_STORAGE_KEY_OPERATOR).await;

        let account_detail_type =
            Self::get_persistent_store_string(state, PERSISTENT_STORAGE_ACCOUNT_DETAIL_TYPE).await;

        let device_type =
            match Self::get_persistent_store_string(state, PERSISTENT_STORAGE_ACCOUNT_DEVICE_TYPE)
                .await
            {
                Some(s) => s,
                None => state.get_device_manifest().get_form_factor(),
            };

        let device_manufacturer = match Self::get_persistent_store_string(
            state,
            PERSISTENT_STORAGE_ACCOUNT_DEVICE_MANUFACTURER,
        )
        .await
        {
            Some(s) => s,
            None => {
                if let Ok(response) = state
                    .get_client()
                    .send_extn_request(DeviceInfoRequest::Make)
                    .await
                {
                    if let Some(ExtnResponse::String(v)) = response.payload.extract() {
                        v
                    } else {
                        "Device.Manufacturer.missing.from.persistent.store".to_string()
                    }
                } else {
                    "Device.Manufacturer.missing.from.persistent.store".to_string()
                }
            }
        };
        let authenticated = Some(true);

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
            context.device_name = Some(device_name);
            context.device_session_id = state.device_session_id.clone().into();
            context.firmware = firmware;
            context.ripple_version = state
                .version
                .clone()
                .unwrap_or(String::from(SEMVER_LIGHTWEIGHT));

            if let Some(t) = timezone {
                context.device_timezone = t;
            }

            context.env = env;
            context.activated = activated;
            context.proposition = proposition;
            context.retailer = retailer;
            context.primary_provider = primary_provider;
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
