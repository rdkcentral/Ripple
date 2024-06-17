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
        device::device_user_grants_data::{GrantExclusionFilter, GrantPolicies},
        distributor::distributor_privacy::DataEventType,
        firebolt::fb_capabilities::{FireboltCap, FireboltPermission},
        storage_property::StorageProperty,
    },
    utils::error::RippleError,
};

use super::{apps::AppManifest, exclusory::ExclusoryImpl, remote_feature::FeatureFlag};
pub const PARTNER_EXCLUSION_REFRESH_TIMEOUT: u32 = 12 * 60 * 60; // 12 hours
pub const METRICS_LOGGING_PERCENTAGE_DEFAULT: u32 = 10;

#[derive(Deserialize, Debug, Clone)]
pub struct RippleConfiguration {
    #[serde(default = "ws_configuration_default")]
    pub ws_configuration: WsConfiguration,
    #[serde(default = "ws_configuration_internal_default")]
    pub internal_ws_configuration: WsConfiguration,
    #[serde(default = "platform_parameters_default")]
    pub platform_parameters: Value,
    pub distribution_id_salt: Option<IdSalt>,
    pub form_factor: String,
    #[serde(default)]
    pub default_values: DefaultValues,
    #[serde(default)]
    pub settings_defaults_per_app: HashMap<String, SettingsDefaults>,
    #[serde(default)]
    pub model_friendly_names: HashMap<String, String>,
    pub distributor_experience_id: String,
    pub distributor_services: Option<Value>,
    pub exclusory: Option<ExclusoryImpl>,
    #[serde(default)]
    pub features: RippleFeatures,
    pub internal_app_id: Option<String>,
    #[serde(default = "default_saved_dir")]
    pub saved_dir: String,
    #[serde(default)]
    pub data_governance: DataGovernanceConfig,
    #[serde(default = "partner_exclusion_refresh_timeout_default")]
    pub partner_exclusion_refresh_timeout: u32,
    #[serde(default = "metrics_logging_percentage_default")]
    pub metrics_logging_percentage: u32,
}

fn partner_exclusion_refresh_timeout_default() -> u32 {
    PARTNER_EXCLUSION_REFRESH_TIMEOUT
}

fn metrics_logging_percentage_default() -> u32 {
    METRICS_LOGGING_PERCENTAGE_DEFAULT
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityConfiguration {
    pub supported: Vec<String>,
    pub grant_policies: Option<HashMap<String, GrantPolicies>>,
    #[serde(default)]
    pub grant_exclusion_filters: Vec<GrantExclusionFilter>,
    #[serde(default)]
    pub dependencies: HashMap<FireboltPermission, Vec<FireboltPermission>>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "camelCase")]
pub struct LifecycleConfiguration {
    #[serde(default = "lc_config_app_ready_timeout_ms_default")]
    pub app_ready_timeout_ms: u64,
    #[serde(default = "lc_config_app_finished_timeout_ms_default")]
    pub app_finished_timeout_ms: u64,
    #[serde(default = "lc_config_max_loaded_apps_default")]
    pub max_loaded_apps: u64,
    #[serde(default = "lc_config_min_available_memory_kb_default")]
    pub min_available_memory_kb: u64,
    #[serde(default)]
    pub prioritized: Vec<String>,
    #[serde(default)]
    pub emit_app_init_events_enabled: bool,
}

pub fn lc_config_app_ready_timeout_ms_default() -> u64 {
    DEFAULT_LIFECYCLE_POLICY.app_ready_timeout_ms
}

pub fn lc_config_app_finished_timeout_ms_default() -> u64 {
    DEFAULT_LIFECYCLE_POLICY.app_finished_timeout_ms
}

pub fn lc_config_max_loaded_apps_default() -> u64 {
    DEFAULT_RENTENTION_POLICY_MAX_RETAINED
}

pub fn lc_config_min_available_memory_kb_default() -> u64 {
    DEFAULT_RENTENTION_POLICY_MIN_AVAILABLE_MEM_KB
}

impl LifecycleConfiguration {
    pub fn is_emit_event_on_app_init_enabled(&self) -> bool {
        self.emit_app_init_events_enabled
    }
}
/// Device manifest contains all the specifications required for coniguration of a Ripple application.
/// Device manifest file should be compliant to the Openrpc schema specified in <https://github.com/rdkcentral/firebolt-configuration>
#[derive(Deserialize, Debug, Clone, Default)]
pub struct DeviceManifest {
    pub configuration: RippleConfiguration,
    pub capabilities: CapabilityConfiguration,
    #[serde(default)]
    pub lifecycle: LifecycleConfiguration,
    pub applications: ApplicationsConfiguration,
}

#[derive(Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct DistributionConfiguration {
    pub library: String,
}

impl Default for DistributionConfiguration {
    fn default() -> Self {
        DistributionConfiguration {
            library: "/etc/firebolt-app-library.json".into(),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
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

#[derive(Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ApplicationsConfiguration {
    #[serde(default)]
    pub distribution: DistributionConfiguration,
    pub defaults: ApplicationDefaultsConfiguration,
    #[serde(default)]
    pub distributor_app_aliases: HashMap<String, String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct WsConfiguration {
    pub enabled: bool,
    pub gateway: String,
}

pub fn ws_configuration_default() -> WsConfiguration {
    WsConfiguration {
        enabled: true,
        gateway: "127.0.0.1:3473".into(),
    }
}

pub fn ws_configuration_internal_default() -> WsConfiguration {
    WsConfiguration {
        enabled: true,
        gateway: "127.0.0.1:3474".into(),
    }
}

pub fn platform_parameters_default() -> Value {
    serde_json::to_value(HashMap::from([("gateway", "ws://127.0.0.1:9998/jsonrpc")]))
        .unwrap_or(Value::Null)
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
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

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecyclePolicy {
    pub app_ready_timeout_ms: u64,
    pub app_finished_timeout_ms: u64,
}

pub const DEFAULT_LIFECYCLE_POLICY: LifecyclePolicy = LifecyclePolicy {
    app_ready_timeout_ms: 30000,
    app_finished_timeout_ms: 2000,
};

pub const DEFAULT_RENTENTION_POLICY_MAX_RETAINED: u64 = 5;
pub const DEFAULT_RENTENTION_POLICY_MIN_AVAILABLE_MEM_KB: u64 = 1024;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AppLibraryEntry {
    pub app_id: String,
    pub manifest: AppManifestLoad,
    pub boot_state: BootState,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum AppManifestLoad {
    Remote(String),
    Local(String),
    Embedded(AppManifest),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DefaultValues {
    #[serde(default = "country_code_default")]
    pub country_code: String,
    #[serde(default = "language_default")]
    pub language: String,
    #[serde(default = "locale_default")]
    pub locale: String,
    #[serde(default = "name_default")]
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
    #[serde(default)]
    pub lifecycle_transition_validate: bool,
    #[serde(default, rename = "mediaProgressAsWatchedEvents")]
    pub media_progress_as_watched_events: bool,
    #[serde(default)]
    pub accessibility_audio_description_settings: bool,
}

pub fn name_default() -> String {
    "Living Room".to_string()
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
#[cfg_attr(test, derive(PartialEq))]
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
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "voice_guidance_speed_default")]
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

impl Default for DefaultValues {
    fn default() -> Self {
        DefaultValues {
            country_code: country_code_default(),
            language: language_default(),
            locale: locale_default(),
            name: name_default(),
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
            lifecycle_transition_validate: false,
            media_progress_as_watched_events: false,
            accessibility_audio_description_settings: false,
        }
    }
}

fn country_code_default() -> String {
    "US".to_string()
}

fn language_default() -> String {
    "en".to_string()
}

fn locale_default() -> String {
    "en-US".to_string()
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

fn voice_guidance_speed_default() -> f32 {
    5.0
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
#[cfg_attr(test, derive(PartialEq))]
pub struct RippleFeatures {
    #[serde(default = "default_privacy_settings_storage_type")]
    pub privacy_settings_storage_type: PrivacySettingsStorageType,
    #[serde(default = "default_intent_validation")]
    pub intent_validation: IntentValidation,
    #[serde(default = "default_cloud_permissions")]
    pub cloud_permissions: bool,
    #[serde(default)]
    pub catalog_uninstalls_enabled: FeatureFlag,
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

#[derive(Deserialize, Serialize, Debug, Clone)]
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

impl Default for DataGovernanceConfig {
    fn default() -> Self {
        DataGovernanceConfig {
            policies: vec![
                DataGovernancePolicy::new(
                    DataEventType::Watched,
                    vec![
                        DataGovernanceSettingTag::new(
                            StorageProperty::AllowPersonalization,
                            false,
                            HashSet::from([
                                "dataPlatform:cet:xvp:personalization:recommendation".into()
                            ]),
                        ),
                        DataGovernanceSettingTag::new(
                            StorageProperty::AllowResumePoints,
                            false,
                            HashSet::from([
                                "dataPlatform:cet:xvp:personalization:continueWatching".into(),
                            ]),
                        ),
                        DataGovernanceSettingTag::new(
                            StorageProperty::AllowWatchHistory,
                            false,
                            HashSet::from([
                                "dataPlatform:cet:xvp:personalization:continueWatching".into(),
                            ]),
                        ),
                        DataGovernanceSettingTag::new(
                            StorageProperty::AllowProductAnalytics,
                            false,
                            HashSet::from(["dataPlatform:cet:xvp:analytics".into()]),
                        ),
                        DataGovernanceSettingTag::new(
                            StorageProperty::AllowBusinessAnalytics,
                            false,
                            HashSet::from(["dataPlatform:cet:xvp:analytics:business".into()]),
                        ),
                    ],
                    false,
                ),
                DataGovernancePolicy::new(
                    DataEventType::BusinessIntelligence,
                    vec![
                        DataGovernanceSettingTag::new(
                            StorageProperty::AllowPersonalization,
                            false,
                            HashSet::from([
                                "dataPlatform:cet:xvp:personalization:recommendation".into()
                            ]),
                        ),
                        DataGovernanceSettingTag::new(
                            StorageProperty::AllowResumePoints,
                            false,
                            HashSet::from([
                                "dataPlatform:cet:xvp:personalization:continueWatching".into(),
                            ]),
                        ),
                        DataGovernanceSettingTag::new(
                            StorageProperty::AllowWatchHistory,
                            false,
                            HashSet::from([
                                "dataPlatform:cet:xvp:personalization:continueWatching".into(),
                            ]),
                        ),
                        DataGovernanceSettingTag::new(
                            StorageProperty::AllowProductAnalytics,
                            false,
                            HashSet::from(["dataPlatform:cet:xvp:analytics".into()]),
                        ),
                        DataGovernanceSettingTag::new(
                            StorageProperty::AllowBusinessAnalytics,
                            false,
                            HashSet::from(["dataPlatform:cet:xvp:analytics:business".into()]),
                        ),
                    ],
                    false,
                ),
            ],
        }
    }
}

impl Default for RippleFeatures {
    fn default() -> Self {
        RippleFeatures {
            privacy_settings_storage_type: default_privacy_settings_storage_type(),
            intent_validation: default_intent_validation(),
            cloud_permissions: default_cloud_permissions(),
            catalog_uninstalls_enabled: Default::default(),
        }
    }
}

fn default_intent_validation() -> IntentValidation {
    IntentValidation::FailOpen
}

fn default_privacy_settings_storage_type() -> PrivacySettingsStorageType {
    PrivacySettingsStorageType::Local
}

fn default_cloud_permissions() -> bool {
    true
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

impl DataGovernancePolicy {
    pub fn new(
        data_type: DataEventType,
        setting_tags: Vec<DataGovernanceSettingTag>,
        drop_on_all_tags: bool,
    ) -> Self {
        DataGovernancePolicy {
            data_type,
            setting_tags,
            drop_on_all_tags,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DataGovernanceSettingTag {
    pub setting: StorageProperty,
    #[serde(default = "default_enforcement_value")]
    pub enforcement_value: bool,
    pub tags: HashSet<String>,
}

impl DataGovernanceSettingTag {
    pub fn new(setting: StorageProperty, enforcement_value: bool, tags: HashSet<String>) -> Self {
        DataGovernanceSettingTag {
            setting,
            enforcement_value,
            tags,
        }
    }
}

impl Default for RippleConfiguration {
    fn default() -> Self {
        Self {
            ws_configuration: Default::default(),
            internal_ws_configuration: Default::default(),
            platform_parameters: Value::Null,
            distribution_id_salt: None,
            form_factor: Default::default(),
            default_values: DefaultValues::default(),
            settings_defaults_per_app: Default::default(),
            model_friendly_names: Default::default(),
            distributor_experience_id: Default::default(),
            distributor_services: None,
            exclusory: None,
            features: Default::default(),
            internal_app_id: None,
            saved_dir: default_saved_dir(),
            data_governance: Default::default(),
            partner_exclusion_refresh_timeout: partner_exclusion_refresh_timeout_default(),
            metrics_logging_percentage: metrics_logging_percentage_default(),
        }
    }
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

    pub fn get_applications_configuration(&self) -> ApplicationsConfiguration {
        self.applications.clone()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub trait Mockable {
        fn mock() -> DeviceManifest
        where
            Self: Sized;
    }
    #[cfg(test)]
    impl Mockable for DeviceManifest {
        fn mock() -> DeviceManifest {
            DeviceManifest {
                configuration: RippleConfiguration {
                    ws_configuration: WsConfiguration {
                        enabled: true,
                        gateway: "127.0.0.1:3473".to_string(),
                    },
                    internal_ws_configuration: WsConfiguration {
                        enabled: true,
                        gateway: "127.0.0.1:3474".to_string(),
                    },
                    platform_parameters: {
                        let mut params = HashMap::new();
                        params.insert(
                            "gateway".to_string(),
                            "ws://127.0.0.1:9998/jsonrpc".to_string(),
                        );
                        serde_json::to_value(params).unwrap()
                    },
                    distribution_id_salt: None,
                    form_factor: "ipstb".to_string(),
                    default_values: DefaultValues {
                        country_code: "US".to_string(),
                        language: "en".to_string(),
                        locale: "en-US".to_string(),
                        name: "Living Room".to_string(),
                        captions: CaptionStyle {
                            enabled: false,
                            font_family: Some("sans-serif".to_string()),
                            font_size: Some(1.0),
                            font_color: Some("#ffffff".to_string()),
                            font_edge: Some("none".to_string()),
                            font_edge_color: Some("#7F7F7F".to_string()),
                            font_opacity: Some(100),
                            background_color: Some("#000000".to_string()),
                            background_opacity: Some(12),
                            window_color: None,
                            window_opacity: None,
                            text_align: Some("center".to_string()),
                            text_align_vertical: Some("middle".to_string()),
                        },
                        additional_info: HashMap::new(),
                        voice: VoiceGuidance {
                            enabled: true,
                            speed: 5.0,
                        },
                        allow_acr_collection: false,
                        allow_app_content_ad_targeting: false,
                        allow_business_analytics: true,
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
                        video_dimensions: vec![1920, 1080],
                        // setting the value to true to simulate manifest override
                        lifecycle_transition_validate: true,
                        media_progress_as_watched_events: true,
                        accessibility_audio_description_settings: false,
                    },
                    settings_defaults_per_app: HashMap::new(),
                    model_friendly_names: {
                        let mut names = HashMap::new();
                        names.insert("RSPPI".to_string(), "Raspberry PI".to_string());
                        names
                    },
                    distributor_experience_id: "0000".to_string(),
                    distributor_services: None,
                    exclusory: None,
                    features: RippleFeatures {
                        privacy_settings_storage_type: PrivacySettingsStorageType::Local,
                        intent_validation: IntentValidation::Fail,
                        cloud_permissions: true,
                        catalog_uninstalls_enabled: FeatureFlag {
                            default: false,
                            remote_key: None,
                        },
                    },
                    internal_app_id: Some("test".to_string()),
                    saved_dir: "/opt/persistent/ripple".to_string(),
                    data_governance: DataGovernanceConfig {
                        policies: Vec::new(),
                    },
                    partner_exclusion_refresh_timeout: 43200,
                    metrics_logging_percentage: 10,
                },
                capabilities: CapabilityConfiguration {
                    supported: vec!["main".to_string()],
                    grant_policies: None,
                    grant_exclusion_filters: vec![GrantExclusionFilter {
                        id: Some("test-id".to_string()),
                        capability: Some("test-cap".to_string()),
                        catalog: Some("test-catalog".to_string()),
                    }],
                    dependencies: HashMap::new(),
                },
                lifecycle: LifecycleConfiguration {
                    app_ready_timeout_ms: 30000,
                    app_finished_timeout_ms: 2000,
                    max_loaded_apps: 5,
                    min_available_memory_kb: 1024,
                    prioritized: Vec::new(),
                    emit_app_init_events_enabled: false,
                },
                applications: ApplicationsConfiguration {
                    distribution: DistributionConfiguration {
                        library: "/etc/firebolt-app-library.json".to_string(),
                    },
                    defaults: ApplicationDefaultsConfiguration {
                        main: "".to_string(),
                        settings: "".to_string(),
                        player: None,
                    },
                    distributor_app_aliases: HashMap::new(),
                },
            }
        }
    }

    #[test]
    fn test_get_internal_app_id() {
        let manifest = DeviceManifest::mock();
        let internal_app_id = manifest.get_internal_app_id();
        assert_eq!(internal_app_id, Some("test".to_string()));
    }

    #[test]
    fn test_get_form_factor() {
        let manifest = DeviceManifest::mock();
        let form_factor = manifest.get_form_factor();
        assert_eq!(form_factor, "ipstb".to_string());
    }

    #[test]
    fn test_get_app_library_path() {
        let manifest = DeviceManifest::mock();
        let app_library_path = manifest.get_app_library_path();
        assert_eq!(
            app_library_path,
            "/etc/firebolt-app-library.json".to_string()
        );
    }

    #[test]
    fn test_get_lifecycle_policy() {
        let manifest = DeviceManifest::mock();
        let lifecycle_policy = manifest.get_lifecycle_policy();

        assert_eq!(lifecycle_policy.app_ready_timeout_ms, 30000);
        assert_eq!(lifecycle_policy.app_finished_timeout_ms, 2000);
    }

    #[test]
    fn test_get_retention_policy() {
        let manifest = DeviceManifest::mock();
        let retention_policy = manifest.get_retention_policy();

        assert_eq!(retention_policy.max_retained, 5);
        assert_eq!(retention_policy.min_available_mem_kb, 1024);
        assert_eq!(retention_policy.always_retained, Vec::<String>::new());
    }

    #[test]
    fn test_get_supported_caps() {
        let manifest = DeviceManifest::mock();
        let supported_caps = manifest.get_supported_caps();

        assert_eq!(supported_caps, vec![FireboltCap::Full("main".to_string())]);
    }

    #[test]
    fn test_get_caps_requiring_grant() {
        let manifest = DeviceManifest::mock();
        let caps_requiring_grant = manifest.get_caps_requiring_grant();

        assert_eq!(caps_requiring_grant, Vec::<String>::new());
    }

    #[test]
    fn test_get_grant_policies() {
        let manifest = DeviceManifest::mock();
        let grant_policies = manifest.get_grant_policies();

        assert_eq!(grant_policies, None);
    }

    #[test]
    fn test_get_grant_exclusion_filters() {
        let manifest = DeviceManifest::mock();
        let grant_exclusion_filters = manifest.get_grant_exclusion_filters();

        assert_eq!(
            grant_exclusion_filters,
            vec![GrantExclusionFilter {
                id: Some("test-id".to_string()),
                capability: Some("test-cap".to_string()),
                catalog: Some("test-catalog".to_string()),
            }]
        );
    }

    #[test]
    fn test_get_distributor_experience_id() {
        let manifest = DeviceManifest::mock();
        let distributor_experience_id = manifest.get_distributor_experience_id();
        assert_eq!(distributor_experience_id, "0000".to_string());
    }

    #[test]
    fn test_get_features() {
        let manifest = DeviceManifest::mock();
        let features = manifest.get_features();

        assert_eq!(
            features,
            RippleFeatures {
                privacy_settings_storage_type: PrivacySettingsStorageType::Local,
                intent_validation: IntentValidation::Fail,
                cloud_permissions: true,
                catalog_uninstalls_enabled: FeatureFlag {
                    default: false,
                    remote_key: None,
                },
            }
        );
    }

    #[test]
    fn test_get_settings_defaults_per_app() {
        let manifest = DeviceManifest::mock();
        let settings_defaults_per_app = manifest.get_settings_defaults_per_app();

        assert_eq!(settings_defaults_per_app, HashMap::new());
    }

    #[test]
    fn test_get_model_friendly_names() {
        let manifest = DeviceManifest::mock();
        let model_friendly_names = manifest.get_model_friendly_names();

        let mut expected_model_friendly_names = HashMap::new();
        expected_model_friendly_names.insert("RSPPI".to_string(), "Raspberry PI".to_string());
        assert_eq!(model_friendly_names, expected_model_friendly_names);
    }

    #[test]
    fn test_get_lifecycle_configuration() {
        let manifest = DeviceManifest::mock();
        let lifecycle_configuration = manifest.get_lifecycle_configuration();

        assert_eq!(
            lifecycle_configuration,
            LifecycleConfiguration {
                app_ready_timeout_ms: 30000,
                app_finished_timeout_ms: 2000,
                max_loaded_apps: 5,
                min_available_memory_kb: 1024,
                prioritized: Vec::new(),
                emit_app_init_events_enabled: false,
            }
        );
    }

    #[test]
    fn test_get_applications_configuration() {
        let manifest = DeviceManifest::mock();
        let applications_configuration = manifest.get_applications_configuration();

        assert_eq!(
            applications_configuration,
            ApplicationsConfiguration {
                distribution: DistributionConfiguration {
                    library: "/etc/firebolt-app-library.json".to_string(),
                },
                defaults: ApplicationDefaultsConfiguration {
                    main: "".to_string(),
                    settings: "".to_string(),
                    player: None,
                },
                distributor_app_aliases: HashMap::new(),
            }
        );
    }

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

    #[test]
    fn test_media_progress_as_watched_events() {
        let manifest = DeviceManifest::mock();
        assert!(
            manifest
                .configuration
                .default_values
                .media_progress_as_watched_events
        );
    }

    #[test]
    fn test_default_media_progress_as_watched_events() {
        // create DefaultValues object by providing only the required fields
        let default_values = serde_json::from_str::<DefaultValues>(
            r#"{
            "country_code": "US",
            "language": "en",
            "locale": "en-US",
            "name": "Living Room"
        }"#,
        )
        .unwrap();
        assert!(!default_values.media_progress_as_watched_events);
    }

    #[test]
    fn test_media_progress_as_watched_events_override() {
        // create DefaultValues object by providing the required fields and mediaProgressAsWatchedEvents
        let default_values = serde_json::from_str::<DefaultValues>(
            r#"{
            "country_code": "US",
            "language": "en",
            "locale": "en-US",
            "name": "Living Room",
            "mediaProgressAsWatchedEvents": true
        }"#,
        )
        .unwrap();
        assert!(default_values.media_progress_as_watched_events);
    }

    #[test]
    fn test_lifecycle_transition_validate() {
        let manifest = DeviceManifest::mock();
        assert!(
            manifest
                .configuration
                .default_values
                .lifecycle_transition_validate
        );
    }

    #[test]
    fn test_default_lifecycle_transition_validate() {
        // create DefaultValues object by providing only the required fields
        let default_values = serde_json::from_str::<DefaultValues>(
            r#"{
            "country_code": "US",
            "language": "en",
            "locale": "en-US",
            "name": "Living Room"
        }"#,
        )
        .unwrap();
        assert!(!default_values.lifecycle_transition_validate);
    }

    #[test]
    fn test_lifecycle_transition_validate_override() {
        // create DefaultValues object by providing the required fields and lifecycle_transition_validate
        let default_values = serde_json::from_str::<DefaultValues>(
            r#"{
            "country_code": "US",
            "language": "en",
            "locale": "en-US",
            "name": "Living Room",
            "lifecycle_transition_validate": true
        }"#,
        )
        .unwrap();
        assert!(default_values.lifecycle_transition_validate);
    }

    #[test]
    fn test_accessibility_audio_desc_settings_default_value() {
        let manifest = DeviceManifest::mock();
        assert!(
            !manifest
                .configuration
                .default_values
                .accessibility_audio_description_settings
        );
    }
}
