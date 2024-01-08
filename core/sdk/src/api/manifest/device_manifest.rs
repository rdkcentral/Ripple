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
        device::{
            device_user_grants_data::{GrantExclusionFilter, GrantPolicies},
            DevicePlatformType,
        },
        distributor::distributor_privacy::DataEventType,
        firebolt::fb_capabilities::{FireboltCap, FireboltPermission},
        storage_property::StorageProperty,
    },
    utils::error::RippleError,
};

use super::{apps::AppManifest, exclusory::ExclusoryImpl};
pub const PARTNER_EXCLUSION_REFRESH_TIMEOUT: u32 = 12 * 60 * 60; // 12 hours

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
    #[serde(default = "partner_exclusion_refresh_timeout_default")]
    pub partner_exclusion_refresh_timeout: u32,
}

fn data_governance_default() -> DataGovernanceConfig {
    DataGovernanceConfig::default()
}

fn partner_exclusion_refresh_timeout_default() -> u32 {
    PARTNER_EXCLUSION_REFRESH_TIMEOUT
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityConfiguration {
    pub supported: Vec<String>,
    pub grant_policies: Option<HashMap<String, GrantPolicies>>,
    #[serde(default)]
    pub grant_exclusion_filters: Vec<GrantExclusionFilter>,
    #[serde(default)]
    pub dependencies: HashMap<FireboltPermission, Vec<FireboltPermission>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleConfiguration {
    pub app_ready_timeout_ms: u64,
    pub app_finished_timeout_ms: u64,
    pub max_loaded_apps: u64,
    pub min_available_memory_kb: u64,
    pub prioritized: Vec<String>,
    #[serde(default)]
    pub emit_app_init_events_enabled: bool,
}

impl LifecycleConfiguration {
    pub fn is_emit_event_on_app_init_enabled(&self) -> bool {
        self.emit_app_init_events_enabled
    }
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
    pub fn get_reserved_application_id(&self, reserved_app_type: &str) -> Option<&str> {
        match reserved_app_type {
            "xrn:firebolt:application-type:main" | "urn:firebolt:apps:main" => Some(&self.main),
            "xrn:firebolt:application-type:settings" | "urn:firebolt:apps:settings" => {
                Some(&self.settings)
            }
            "xrn:firebolt:application-type:player" | "urn:firebolt:apps:player" => {
                self.player.as_deref().or(Some(""))
            }
            _ => None,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApplicationsConfiguration {
    pub distribution: DistributionConfiguration,
    pub defaults: ApplicationDefaultsConfiguration,
    #[serde(default)]
    pub distributor_app_aliases: HashMap<String, String>,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DefaultValues {
    pub country_code: String,
    pub language: String,
    pub locale: String,
    pub name: String,
    #[serde(default = "captions_default")]
    pub captions: CaptionStyle,
    #[serde(default = "additional_info_default")]
    pub additional_info: HashMap<String, String>,
    #[serde(default = "voice_guidance_default")]
    pub voice: VoiceGuidance,
    #[serde(default)]
    pub allow_acr_collection: bool,
    #[serde(default)]
    pub allow_app_content_ad_targeting: bool,
    #[serde(default = "default_business_analytics")]
    pub allow_business_analytics: bool,
    #[serde(default)]
    pub allow_camera_analytics: bool,
    #[serde(default)]
    pub allow_personalization: bool,
    #[serde(default)]
    pub allow_primary_browse_ad_targeting: bool,
    #[serde(default)]
    pub allow_primary_content_ad_targeting: bool,
    #[serde(default)]
    pub allow_product_analytics: bool,
    #[serde(default)]
    pub allow_remote_diagnostics: bool,
    #[serde(default)]
    pub allow_resume_points: bool,
    #[serde(default)]
    pub allow_unentitled_personalization: bool,
    #[serde(default)]
    pub allow_unentitled_resume_points: bool,
    #[serde(default)]
    pub allow_watch_history: bool,
    #[serde(default = "default_skip_restriction")]
    pub skip_restriction: String,
    #[serde(default = "default_video_dimensions")]
    pub video_dimensions: Vec<i32>,
}

fn additional_info_default() -> HashMap<String, String> {
    HashMap::default()
}

pub fn default_business_analytics() -> bool {
    true
}

pub fn default_skip_restriction() -> String {
    "none".to_owned()
}

pub fn default_video_dimensions() -> Vec<i32> {
    vec![1920, 1080]
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct SettingsDefaults {
    pub postal_code: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CaptionStyle {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_edge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_edge_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_opacity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_opacity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_opacity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align_vertical: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct VoiceGuidance {
    pub enabled: bool,
    pub speed: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
        additional_info: additional_info_default(),
        allow_acr_collection: false,
        allow_app_content_ad_targeting: false,
        allow_business_analytics: default_business_analytics(),
        allow_camera_analytics: false,
        allow_personalization: false,
        allow_primary_browse_ad_targeting: false,
        allow_primary_content_ad_targeting: false,
        allow_product_analytics: false,
        allow_remote_diagnostics: false,
        allow_resume_points: false,
        allow_unentitled_personalization: false,
        allow_unentitled_resume_points: false,
        allow_watch_history: false,
        skip_restriction: "none".to_string(),
        video_dimensions: default_video_dimensions(),
    }
}

fn captions_default() -> CaptionStyle {
    CaptionStyle {
        enabled: false,
        font_family: None,
        font_size: None,
        font_color: None,
        font_edge: None,
        font_edge_color: None,
        font_opacity: None,
        background_color: None,
        background_opacity: None,
        window_color: None,
        window_opacity: None,
        text_align: None,
        text_align_vertical: None,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PrivacySettingsStorageType {
    Local,
    Cloud,
    Sync,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RippleFeatures {
    #[serde(default = "default_privacy_settings_storage_type")]
    pub privacy_settings_storage_type: PrivacySettingsStorageType,
    #[serde(default = "default_intent_validation")]
    pub intent_validation: IntentValidation,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum IntentValidation {
    Fail,
    FailOpen,
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

fn default_ripple_features() -> RippleFeatures {
    RippleFeatures {
        privacy_settings_storage_type: default_privacy_settings_storage_type(),
        intent_validation: default_intent_validation(),
    }
}

fn default_intent_validation() -> IntentValidation {
    IntentValidation::FailOpen
}

fn default_privacy_settings_storage_type() -> PrivacySettingsStorageType {
    PrivacySettingsStorageType::Local
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
            if let Ok(contents) = fs::read_to_string(p) {
                return Self::load_from_content(contents);
            }
        }
        info!("No device manifest found in {}", path);
        Err(RippleError::MissingInput)
    }

    pub fn load_from_content(contents: String) -> Result<(String, DeviceManifest), RippleError> {
        match serde_json::from_str::<DeviceManifest>(&contents) {
            Ok(manifest) => Ok((contents, manifest)),
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
            return policies.into_keys().collect();
        }
        Vec::new()
    }

    pub fn get_grant_policies(&self) -> Option<HashMap<String, GrantPolicies>> {
        self.clone().capabilities.grant_policies
    }

    pub fn get_grant_exclusion_filters(&self) -> Vec<GrantExclusionFilter> {
        self.clone().capabilities.grant_exclusion_filters
    }

    pub fn get_distributor_experience_id(&self) -> String {
        self.configuration.distributor_experience_id.clone()
    }

    pub fn get_features(&self) -> RippleFeatures {
        self.configuration.features.clone()
    }

    pub fn get_settings_defaults_per_app(&self) -> HashMap<String, SettingsDefaults> {
        self.configuration.clone().settings_defaults_per_app
    }

    pub fn get_model_friendly_names(&self) -> HashMap<String, String> {
        self.configuration.model_friendly_names.clone()
    }

    pub fn get_lifecycle_configuration(&self) -> LifecycleConfiguration {
        self.lifecycle.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn check_default_features() {
        if let Ok(v) = serde_json::from_str::<RippleFeatures>("{}") {
            assert!(matches!(v.intent_validation, IntentValidation::FailOpen));
            assert!(matches!(
                v.privacy_settings_storage_type,
                PrivacySettingsStorageType::Local
            ));
        }

        if let Ok(v) =
            serde_json::from_str::<RippleFeatures>("{\"privacy_settings_storage_type\": \"sync\"}")
        {
            assert!(matches!(v.intent_validation, IntentValidation::FailOpen));
            assert!(matches!(
                v.privacy_settings_storage_type,
                PrivacySettingsStorageType::Sync
            ));
        }

        if let Ok(v) = serde_json::from_str::<RippleFeatures>("{\"intent_validation\": \"fail\"}") {
            assert!(matches!(v.intent_validation, IntentValidation::Fail));
            assert!(matches!(
                v.privacy_settings_storage_type,
                PrivacySettingsStorageType::Local
            ));
        }
    }
}
