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

use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use crate::{
    api::{
        device::{device_user_grants_data::GrantPolicies, DevicePlatformType},
        distributor::distributor_privacy::DataEventType,
        firebolt::fb_capabilities::FireboltCap,
        storage_property::StorageProperty,
    },
    utils::error::RippleError,
};

use super::{apps::AppManifest, exclusory::ExclusoryImpl};

#[derive(Deserialize, Debug, Clone)]
pub struct RippleConfiguration {
    pub ws_configuration: WsConfiguration,
    pub internal_ws_configuration: WsConfiguration,
    pub platform: DevicePlatformType,
    pub platform_parameters: Value,
    pub distribution_platform: String,
    pub distribution_id_salt: Option<IdSalt>,
    pub form_factor: String,
    #[serde(default = "default_values_default")]
    pub default_values: DefaultValues,
    #[serde(default = "settings_defaults_per_app_default")]
    pub settings_defaults_per_app: HashMap<String, SettingsDefaults>,
    #[serde(default = "model_friendly_names_default")]
    pub model_friendly_names: HashMap<String, String>,
    pub distributor_experience_id: String,
    pub distributor_services: Option<Value>,
    pub exclusory: Option<ExclusoryImpl>,
    #[serde(default = "default_ripple_features")]
    pub features: RippleFeatures,
    pub internal_app_id: Option<String>,
    #[serde(default = "default_saved_dir")]
    pub saved_dir: String,
    #[serde(default = "data_governance_default")]
    pub data_governance: DataGovernanceConfig,
    pub partner_exclusion_refresh_timeout: Option<u32>,
}

fn data_governance_default() -> DataGovernanceConfig {
    DataGovernanceConfig::default()
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityConfiguration {
    pub supported: Vec<String>,
    pub grant_policies: Option<HashMap<String, GrantPolicies>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleConfiguration {
    pub app_ready_timeout_ms: u64,
    pub app_finished_timeout_ms: u64,
    pub max_loaded_apps: u64,
    pub min_available_memory_kb: u64,
    pub prioritized: Vec<String>,
}

/// Device manifest contains all the specifications required for coniguration of a Ripple application.
/// Device manifest file should be compliant to the Openrpc schema specified in <https://github.com/rdkcentral/firebolt-configuration>
#[derive(Deserialize, Debug, Clone)]
pub struct DeviceManifest {
    pub configuration: RippleConfiguration,
    pub capabilities: CapabilityConfiguration,
    pub lifecycle: LifecycleConfiguration,
    pub applications: ApplicationsConfiguration,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DistributionConfiguration {
    pub library: String,
    pub catalog: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApplicationDefaultsConfiguration {
    #[serde(rename = "xrn:firebolt:application-type:main")]
    pub main: String,
    #[serde(rename = "xrn:firebolt:application-type:settings")]
    pub settings: String,
    #[serde(
        rename = "xrn:firebolt:application-type:player",
        skip_serializing_if = "Option::is_none"
    )]
    pub player: Option<String>,
}

impl ApplicationDefaultsConfiguration {
    pub fn get_reserved_application_id(&self, field_name: &str) -> Option<&str> {
        match field_name {
            "xrn:firebolt:application-type:main" => Some(&self.main),
            "xrn:firebolt:application-type:settings" => Some(&self.settings),
            "xrn:firebolt:application-type:player" => {
                self.player.as_ref().map(|p| p.as_str()).or(Some(""))
            }
            _ => None,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApplicationsConfiguration {
    pub distribution: DistributionConfiguration,
    pub defaults: ApplicationDefaultsConfiguration,
}

#[derive(Deserialize, Debug, Clone)]
pub struct WsConfiguration {
    pub enabled: bool,
    pub gateway: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPolicy {
    pub max_retained: u64,
    pub min_available_mem_kb: u64,
    pub always_retained: Vec<String>,
    // TODO: max_retained and always_retained are related in that max_retained can be no
    // smaller than always_retained.len(). Unit tests to validate. If we move forward
    // with supporting minimal available memory we should also consider implmenting a
    // memory monitor instead of only checking memory as apps are loaded.
}

pub const DEFAULT_RETENTION_POLICY: RetentionPolicy = RetentionPolicy {
    max_retained: 0,
    min_available_mem_kb: 0,
    always_retained: Vec::new(),
};

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecyclePolicy {
    pub app_ready_timeout_ms: u64,
    pub app_finished_timeout_ms: u64,
}

pub const DEFAULT_LIFECYCLE_POLICY: LifecyclePolicy = LifecyclePolicy {
    app_ready_timeout_ms: 30000,
    app_finished_timeout_ms: 2000,
};

#[derive(Deserialize, Debug, Clone)]
pub struct IdSalt {
    pub algorithm: Option<String>,
    pub magic: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppsDistributionConfiguration {
    pub platform: String,
    pub tenant: String,
    // TODO: Next iteration have this for each app
    pub default_id_salt: Option<IdSalt>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppLibraryEntry {
    pub app_id: String,
    pub manifest: AppManifestLoad,
    pub boot_state: BootState,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum AppManifestLoad {
    Remote(String),
    Local(String),
    Embedded(AppManifest),
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct DefaultValues {
    pub country_code: String,
    pub language: String,
    pub locale: String,
    pub name: String,
    #[serde(default = "captions_default")]
    pub captions: CaptionStyle,
    #[serde(default = "voice_guidance_default")]
    pub voice: VoiceGuidance,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct SettingsDefaults {
    pub postal_code: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct CaptionStyle {
    pub enabled: bool,
    pub font_family: String,
    pub font_size: f32,
    pub font_color: String,
    pub font_edge: String,
    pub font_edge_color: String,
    pub font_opacity: u32,
    pub background_color: String,
    pub background_opacity: u32,
    pub text_align: String,
    pub text_align_vertical: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct VoiceGuidance {
    pub enabled: bool,
    pub speed: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BootState {
    Inactive,
    Foreground,
    Unloaded,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum AppLauncherMode {
    External,
    Internal,
}

fn model_friendly_names_default() -> HashMap<String, String> {
    HashMap::default()
}

fn settings_defaults_per_app_default() -> HashMap<String, SettingsDefaults> {
    HashMap::default()
}

fn default_values_default() -> DefaultValues {
    DefaultValues {
        country_code: "US".to_string(),
        language: "en".to_string(),
        locale: "en-US".to_string(),
        name: "Living Room".to_string(),
        captions: captions_default(),
        voice: voice_guidance_default(),
    }
}

fn captions_default() -> CaptionStyle {
    CaptionStyle {
        enabled: false,
        font_family: "sans-serif".to_string(),
        font_size: 1.0,
        font_color: "#ffffff".to_string(),
        font_edge: "none".to_string(),
        font_edge_color: "#7F7F7F".to_string(),
        font_opacity: 100,
        background_color: "#000000".to_string(),
        background_opacity: 12,
        text_align: "center".to_string(),
        text_align_vertical: "middle".to_string(),
    }
}

fn voice_guidance_default() -> VoiceGuidance {
    VoiceGuidance {
        enabled: false,
        speed: 5.0,
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CloudService {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppAuthorizationRules {
    pub app_ignore_rules: HashMap<String, Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PrivacySettingsStorageType {
    Local,
    Cloud,
    Sync,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RippleFeatures {
    pub app_scoped_device_tokens: bool,
    pub privacy_settings_storage_type: PrivacySettingsStorageType,
}

fn default_ripple_features() -> RippleFeatures {
    RippleFeatures {
        app_scoped_device_tokens: false,
        privacy_settings_storage_type: PrivacySettingsStorageType::Local,
    }
}

fn default_saved_dir() -> String {
    String::from("/opt/persistent/ripple")
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct DataGovernanceConfig {
    policies: Vec<DataGovernancePolicy>,
}

impl DataGovernanceConfig {
    pub fn get_policy(&self, data_type: DataEventType) -> Option<DataGovernancePolicy> {
        self.policies
            .iter()
            .find(|p| p.data_type == data_type)
            .cloned()
    }
}

pub fn default_enforcement_value() -> bool {
    false
}

pub fn default_drop_on_all_tags() -> bool {
    true
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DataGovernancePolicy {
    pub data_type: DataEventType,
    pub setting_tags: Vec<DataGovernanceSettingTag>,
    #[serde(default = "default_drop_on_all_tags")]
    pub drop_on_all_tags: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DataGovernanceSettingTag {
    pub setting: StorageProperty,
    #[serde(default = "default_enforcement_value")]
    pub enforcement_value: bool,
    pub tags: HashSet<String>,
}

impl DeviceManifest {
    pub fn load(path: String) -> Result<(String, DeviceManifest), RippleError> {
        info!("Trying to load device manifest from path={}", path);
        if let Some(p) = Path::new(&path).to_str() {
            if let Ok(contents) = fs::read_to_string(&p) {
                return Self::load_from_content(contents);
            }
        }
        info!("No device manifest found in {}", path);
        Err(RippleError::MissingInput)
    }

    pub fn load_from_content(contents: String) -> Result<(String, DeviceManifest), RippleError> {
        match serde_json::from_str::<DeviceManifest>(&contents) {
            Ok(manifest) => Ok((String::from(contents), manifest)),
            Err(err) => {
                warn!("{:?} could not load device manifest", err);
                Err(RippleError::InvalidInput)
            }
        }
    }

    /// Provides the device platform information from the device manifest
    /// as this value is read from a file loaded dynamically during runtime the response
    /// provided will always be a result which can have an error. Handler should panic if
    /// no valid platform type is provided.
    pub fn get_device_platform(&self) -> DevicePlatformType {
        self.configuration.platform.clone()
    }

    pub fn get_web_socket_enabled(&self) -> bool {
        self.configuration.ws_configuration.enabled
    }

    pub fn get_internal_ws_enabled(&self) -> bool {
        self.configuration.internal_ws_configuration.enabled
    }

    pub fn get_ws_gateway_host(&self) -> String {
        self.configuration.ws_configuration.gateway.clone()
    }

    pub fn get_internal_gateway_host(&self) -> String {
        self.configuration.internal_ws_configuration.gateway.clone()
    }

    pub fn get_internal_app_id(&self) -> Option<String> {
        self.configuration.internal_app_id.clone()
    }

    pub fn get_form_factor(&self) -> String {
        self.configuration.form_factor.clone()
    }

    /// Get path to the app library file
    pub fn get_app_library_path(&self) -> String {
        self.applications.distribution.library.clone()
    }

    pub fn get_lifecycle_policy(&self) -> LifecyclePolicy {
        LifecyclePolicy {
            app_ready_timeout_ms: self.lifecycle.app_ready_timeout_ms,
            app_finished_timeout_ms: self.lifecycle.app_finished_timeout_ms,
        }
    }

    pub fn get_retention_policy(&self) -> RetentionPolicy {
        RetentionPolicy {
            max_retained: self.lifecycle.max_loaded_apps,
            min_available_mem_kb: self.lifecycle.min_available_memory_kb,
            always_retained: self.lifecycle.prioritized.clone(),
        }
    }

    pub fn get_supported_caps(&self) -> Vec<FireboltCap> {
        FireboltCap::from_vec_string(self.clone().capabilities.supported)
    }

    pub fn get_caps_requiring_grant(&self) -> Vec<String> {
        if let Some(policies) = self.clone().capabilities.grant_policies {
            return policies.clone().into_keys().collect();
        }
        Vec::new()
    }

    pub fn get_grant_policies(&self) -> Option<HashMap<String, GrantPolicies>> {
        self.clone().capabilities.grant_policies
    }
    pub fn get_distributor_experience_id(&self) -> String {
        self.configuration.distributor_experience_id.clone()
    }

    pub fn get_features(&self) -> RippleFeatures {
        self.configuration.features.clone()
    }
}
