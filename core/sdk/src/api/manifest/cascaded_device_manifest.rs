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
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::api::{
    device::device_user_grants_data::{
        AutoApplyPolicy, GrantExclusionFilter, GrantPolicies, GrantPolicy, GrantPrivacySetting,
        GrantRequirements, GrantStep,
    },
    distributor::distributor_privacy::DataEventType,
    firebolt::fb_capabilities::FireboltPermission,
    storage_property::StorageProperty,
};

use super::{
    apps::AppManifest,
    device_manifest::{
        ApplicationDefaultsConfiguration, ApplicationsConfiguration, CapabilityConfiguration,
        CaptionStyle, DataGovernanceConfig, DataGovernancePolicy, DataGovernanceSettingTag,
        DefaultValues, DeviceManifest, DistributionConfiguration, IdSalt, IntentValidation,
        InternetMonitoringConfiguration, LifecycleConfiguration, PrivacySettingsStorageType,
        RippleConfiguration, RippleFeatures, VoiceGuidance, WsConfiguration,
    },
    exclusory::{AppAuthorizationRules, ExclusoryImpl},
    remote_feature::FeatureFlag,
    MergeConfig,
};

/// Device manifest contains all the specifications required for coniguration of a Ripple application.
/// Device manifest file should be compliant to the Openrpc schema specified in <https://github.com/rdkcentral/firebolt-configuration>
#[derive(Deserialize, Debug, Clone)]
pub struct CascadedDeviceManifest {
    pub configuration: Option<CascadedRippleConfiguration>,
    pub capabilities: Option<CascadedCapabilityConfiguration>,
    pub lifecycle: Option<CascadedLifecycleConfiguration>,
    pub applications: Option<CascadedApplicationsConfiguration>,
}

impl MergeConfig<CascadedDeviceManifest> for DeviceManifest {
    fn merge_config(&mut self, cascaded: CascadedDeviceManifest) {
        if let Some(cas_configuration) = cascaded.configuration {
            self.configuration.merge_config(cas_configuration);
        }
        if let Some(cas_capabilities) = cascaded.capabilities {
            self.capabilities.merge_config(cas_capabilities);
        }
        if let Some(cas_lifecycle) = cascaded.lifecycle {
            self.lifecycle.merge_config(cas_lifecycle);
        }
        if let Some(cas_applications) = cascaded.applications {
            self.applications.merge_config(cas_applications);
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CascadedRippleConfiguration {
    pub log_signal_log_level: Option<String>,
    pub ws_configuration: Option<WsConfiguration>,
    pub internal_ws_configuration: Option<WsConfiguration>,
    pub platform_parameters: Option<Value>,
    pub distribution_id_salt: Option<IdSalt>,
    pub form_factor: Option<String>,
    pub default_values: Option<CascadedDefaultValues>,
    pub model_friendly_names: Option<HashMap<String, String>>,
    pub distributor_experience_id: Option<String>,
    pub distributor_services: Option<Value>,
    pub exclusory: Option<CascadedExclusoryImpl>,
    pub features: Option<CascadedRippleFeatures>,
    pub internal_app_id: Option<String>,
    pub saved_dir: Option<String>,
    pub data_governance: Option<CascadedDataGovernanceConfig>,
    pub partner_exclusion_refresh_timeout: Option<u32>,
    pub metrics_logging_percentage: Option<u32>,
    pub internet_monitoring_configuration: Option<InternetMonitoringConfiguration>,
}

impl MergeConfig<CascadedRippleConfiguration> for RippleConfiguration {
    fn merge_config(&mut self, cascaded: CascadedRippleConfiguration) {
        if let Some(cas_log_signal_log_level) = cascaded.log_signal_log_level {
            self.log_signal_log_level = cas_log_signal_log_level
        }
        if let Some(cas_ws_configuration) = cascaded.ws_configuration {
            self.ws_configuration = cas_ws_configuration
        }
        if let Some(cas_internal_ws_configuration) = cascaded.internal_ws_configuration {
            self.internal_ws_configuration = cas_internal_ws_configuration
        }
        if let Some(cas_platform_parameters) = cascaded.platform_parameters {
            self.platform_parameters = cas_platform_parameters
        }
        if let Some(cas_distribution_id_salt) = cascaded.distribution_id_salt {
            self.distribution_id_salt = Some(cas_distribution_id_salt)
        }
        if let Some(cas_form_factor) = cascaded.form_factor {
            self.form_factor = cas_form_factor
        }
        if let Some(cas_default_values) = cascaded.default_values {
            self.default_values.merge_config(cas_default_values);
        }
        if let Some(cas_model_friendly_names) = cascaded.model_friendly_names {
            self.model_friendly_names.extend(cas_model_friendly_names);
        }
        if let Some(cas_distributor_experience_id) = cascaded.distributor_experience_id {
            self.distributor_experience_id = cas_distributor_experience_id
        }
        if let Some(cas_distributor_services) = cascaded.distributor_services {
            self.distributor_services = Some(cas_distributor_services)
        }
        if let Some(cas_exclusory) = cascaded.exclusory {
            if self.exclusory.is_none() {
                // TODO: need map cascaded struct into default struct and assign here
                // Eg: need to copy CascadedExclusoryImpl data into CascadedExclusoryImpl (both are diffrent structure)
            } else {
                self.exclusory.clone().unwrap().merge_config(cas_exclusory);
            }
        }
        if let Some(cas_features) = cascaded.features {
            self.features.merge_config(cas_features)
        }
        if let Some(cas_internal_app_id) = cascaded.internal_app_id {
            self.internal_app_id = Some(cas_internal_app_id)
        }
        if let Some(cas_saved_dir) = cascaded.saved_dir {
            self.saved_dir = cas_saved_dir
        }
        if let Some(cas_partner_exclusion_refresh_timeout) =
            cascaded.partner_exclusion_refresh_timeout
        {
            self.partner_exclusion_refresh_timeout = cas_partner_exclusion_refresh_timeout
        }
        if let Some(cas_metrics_logging_percentage) = cascaded.metrics_logging_percentage {
            self.metrics_logging_percentage = cas_metrics_logging_percentage
        }
        if let Some(cas_internet_monitering_conf) = cascaded.internet_monitoring_configuration {
            self.internet_monitoring_configuration = cas_internet_monitering_conf;
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CascadedCapabilityConfiguration {
    pub supported: Option<Vec<String>>,
    pub grant_policies: Option<HashMap<String, CascadedGrantPolicies>>,
    pub grant_exclusion_filters: Option<Vec<GrantExclusionFilter>>,
    pub dependencies: Option<HashMap<FireboltPermission, Vec<FireboltPermission>>>,
}

impl MergeConfig<CascadedCapabilityConfiguration> for CapabilityConfiguration {
    fn merge_config(&mut self, cascaded: CascadedCapabilityConfiguration) {
        // Merge supported capabilities (append and deduplicate if other is Some)
        if let Some(cas_supported) = cascaded.supported {
            self.supported.extend(cas_supported);
            self.supported.sort();
            self.supported.dedup();
        }

        // Merge grant policies
        if let Some(_cas_grant_policies) = cascaded.grant_policies {
            // TODO:
        }

        // Merge grant exclusion filters (append if other is Some)
        if let Some(other_filters) = cascaded.grant_exclusion_filters {
            self.grant_exclusion_filters.extend(other_filters);
        }

        // Merge dependencies
        if let Some(cas_dependencies) = cascaded.dependencies {
            for (key, other_dependencies) in cas_dependencies {
                self.dependencies
                    .entry(key)
                    .or_default()
                    .extend(other_dependencies);
            }
        }
    }
}

//TODO: need to impl GrantPolicy

#[derive(Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CascadedGrantPolicies {
    #[serde(rename = "use")]
    pub use_: Option<GrantPolicy>,
    pub manage: Option<GrantPolicy>,
    pub provide: Option<GrantPolicy>,
}

impl MergeConfig<CascadedGrantPolicies> for GrantPolicies {
    fn merge_config(&mut self, cascaded: CascadedGrantPolicies) {
        if let Some(cas_use) = cascaded.use_ {
            self.use_ = Some(cas_use)
        }
        if let Some(cas_manage) = cascaded.manage {
            self.manage = Some(cas_manage)
        }
        if let Some(cas_provide) = cascaded.provide {
            self.provide = Some(cas_provide)
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CascadedGrantExclusionFilter {
    pub capability: Option<String>,
    pub id: Option<String>,
    pub catalog: Option<String>,
}

impl MergeConfig<CascadedGrantExclusionFilter> for GrantExclusionFilter {
    fn merge_config(&mut self, cascaded: CascadedGrantExclusionFilter) {
        if let Some(cas_capability) = cascaded.capability {
            self.capability = Some(cas_capability)
        }
        if let Some(cas_id) = cascaded.id {
            self.id = Some(cas_id)
        }
        if let Some(cas_catalog) = cascaded.catalog {
            self.catalog = Some(cas_catalog)
        }
    }
}

// #[derive(Deserialize, Debug, Clone)]
// #[cfg_attr(test, derive(PartialEq))]
// #[serde(rename_all = "camelCase")]
// pub struct CascadedGrantPolicy {
//     pub evaluate_at: Option<Vec<EvaluateAt>>,
//     pub options: Option<Vec<CascadedGrantRequirements>>,
//     pub scope: Option<GrantScope>,
//     pub lifespan: Option<GrantLifespan>,
//     pub overridable: Option<bool>,
//     pub lifespan_ttl: Option<u64>,
//     pub privacy_setting: Option<CascadedGrantPrivacySetting>,
//     pub persistence: Option<PolicyPersistenceType>,
// }

// impl MergeConfig<CascadedGrantPolicy> for GrantPolicy {
//     fn merge_config(&mut self, other: CascadedGrantPolicy) {
//         if let Some(evaluate_at) = other.evaluate_at {
//             self.evaluate_at.extend(evaluate_at);
//             self.evaluate_at.dedup();
//         }
//         if let Some(other_options) = other.options {
//             for cascaded_requirements in other_options {
//                 // Try to find a matching existing requirements (you might need a more specific key)
//                 if let Some(existing_requirements) = self.options.iter_mut().find(|req| req.steps.iter().any(|step| cascaded_requirements.steps.as_ref().map_or(false, |cs| cs.iter().any(|cs_step| cs_step.capability == Some(step.capability.clone()))))) {
//                     existing_requirements.merge_config(cascaded_requirements);
//                 } else {
//                     let new_requirements = GrantRequirements {
//                         steps: cascaded_requirements.steps.unwrap_or_default().into_iter().filter_map(|cs| cs.capability.map(|cap| GrantStep {
//                             capability: cap,
//                             configuration: Some(cs.configuration.unwrap_or_default()),
//                         })).collect(),
//                     };
//                     self.options.push(new_requirements);
//                 }
//             }
//         }
//         if let Some(scope) = other.scope {
//             self.scope = scope;
//         }
//         if let Some(lifespan) = other.lifespan {
//             self.lifespan = lifespan;
//         }
//         if let Some(overridable) = other.overridable {
//             self.overridable = overridable;
//         }
//         if let Some(lifespan_ttl) = other.lifespan_ttl {
//             self.lifespan_ttl = Some(lifespan_ttl);
//         }
//         if let Some(privacy_setting) = other.privacy_setting {
//             todo!()
//         }
//         if let Some(persistence) = other.persistence {
//             self.persistence = persistence;
//         }
//     }
// }

#[derive(Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "camelCase")]
pub struct CascadedGrantRequirements {
    pub steps: Option<Vec<CascadedGrantStep>>,
}

impl MergeConfig<CascadedGrantRequirements> for GrantRequirements {
    fn merge_config(&mut self, other: CascadedGrantRequirements) {
        if let Some(other_steps) = other.steps {
            for cascaded_step in other_steps {
                // Try to find a matching existing step (you might need a more specific key)
                if let Some(existing_step) = self.steps.iter_mut().find(|step| {
                    step.capability.as_str()
                        == cascaded_step.capability.as_deref().unwrap_or_default()
                }) {
                    existing_step.merge_config(cascaded_step);
                } else if let Some(capability) = cascaded_step.capability {
                    self.steps.push(GrantStep {
                        capability,
                        configuration: Some(cascaded_step.configuration.unwrap_or_default()),
                    });
                }
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "camelCase")]
pub struct CascadedGrantStep {
    pub capability: Option<String>,
    pub configuration: Option<Value>,
}

impl MergeConfig<CascadedGrantStep> for GrantStep {
    fn merge_config(&mut self, cascaded: CascadedGrantStep) {
        if let Some(cas_capability) = cascaded.capability {
            self.capability = cas_capability
        }
        if let Some(cas_configuration) = cascaded.configuration {
            self.configuration = Some(cas_configuration)
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "camelCase")]
pub struct CascadedGrantPrivacySetting {
    pub property: Option<String>,
    pub auto_apply_policy: Option<AutoApplyPolicy>,
    pub update_property: Option<bool>,
}

impl MergeConfig<CascadedGrantPrivacySetting> for GrantPrivacySetting {
    fn merge_config(&mut self, cascaded: CascadedGrantPrivacySetting) {
        if let Some(ca_property) = cascaded.property {
            self.property = ca_property
        }
        if let Some(cas_auto_apply_policy) = cascaded.auto_apply_policy {
            self.auto_apply_policy = cas_auto_apply_policy
        }
        if let Some(ca_update_property) = cascaded.update_property {
            self.update_property = ca_update_property
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "camelCase")]
pub struct CascadedLifecycleConfiguration {
    pub app_ready_timeout_ms: Option<u64>,
    pub app_finished_timeout_ms: Option<u64>,
    pub max_loaded_apps: Option<u64>,
    pub min_available_memory_kb: Option<u64>,
    pub prioritized: Option<Vec<String>>,
    pub emit_app_init_events_enabled: Option<bool>,
    pub emit_navigate_on_activate: Option<bool>,
}

impl MergeConfig<CascadedLifecycleConfiguration> for LifecycleConfiguration {
    fn merge_config(&mut self, cascaded: CascadedLifecycleConfiguration) {
        if let Some(cas_app_ready_timeout_ms) = cascaded.app_ready_timeout_ms {
            self.app_ready_timeout_ms = cas_app_ready_timeout_ms
        }
        if let Some(cas_app_finished_timeout_ms) = cascaded.app_finished_timeout_ms {
            self.app_finished_timeout_ms = cas_app_finished_timeout_ms
        }
        if let Some(cas_max_loaded_apps) = cascaded.max_loaded_apps {
            self.max_loaded_apps = cas_max_loaded_apps
        }
        if let Some(cas_min_available_memory_kb) = cascaded.min_available_memory_kb {
            self.min_available_memory_kb = cas_min_available_memory_kb
        }
        if let Some(cas_prioritized) = cascaded.prioritized {
            self.prioritized.extend(cas_prioritized);
        }
        if let Some(cas_emit_app_init_events_enabled) = cascaded.emit_app_init_events_enabled {
            self.emit_app_init_events_enabled = cas_emit_app_init_events_enabled
        }
        if let Some(cas_emit_navigate_on_activate) = cascaded.emit_navigate_on_activate {
            self.emit_navigate_on_activate = cas_emit_navigate_on_activate
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CascadedApplicationsConfiguration {
    pub distribution: Option<CascadedDistributionConfiguration>,
    pub defaults: Option<CascadedApplicationDefaultsConfiguration>,
    pub distributor_app_aliases: Option<HashMap<String, String>>,
}

impl MergeConfig<CascadedApplicationsConfiguration> for ApplicationsConfiguration {
    fn merge_config(&mut self, cascaded: CascadedApplicationsConfiguration) {
        if let Some(cas_distribution) = cascaded.distribution {
            self.distribution.merge_config(cas_distribution);
        }
        if let Some(cas_defaults) = cascaded.defaults {
            self.defaults.merge_config(cas_defaults);
        }
        if let Some(cas_distributor_app_aliases) = cascaded.distributor_app_aliases {
            self.distributor_app_aliases
                .extend(cas_distributor_app_aliases);
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CascadedDistributionConfiguration {
    pub library: Option<String>,
}

impl MergeConfig<CascadedDistributionConfiguration> for DistributionConfiguration {
    fn merge_config(&mut self, cascaded: CascadedDistributionConfiguration) {
        if let Some(cas_library) = cascaded.library {
            self.library = cas_library
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CascadedApplicationDefaultsConfiguration {
    pub main: Option<String>,
    pub settings: Option<String>,
    pub player: Option<String>,
}

impl MergeConfig<CascadedApplicationDefaultsConfiguration> for ApplicationDefaultsConfiguration {
    fn merge_config(&mut self, cascaded: CascadedApplicationDefaultsConfiguration) {
        if let Some(cas_main) = cascaded.main {
            self.main = cas_main
        }
        if let Some(cas_settings) = cascaded.settings {
            self.settings = cas_settings
        }
        if let Some(cas_player) = cascaded.player {
            self.player = Some(cas_player)
        }
    }
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
pub struct CascadedDefaultValues {
    pub country_code: Option<String>,
    pub language: Option<String>,
    pub locale: Option<String>,
    pub name: Option<String>,
    pub captions: Option<CaptionStyle>,
    pub additional_info: Option<HashMap<String, String>>,
    pub voice: Option<VoiceGuidance>,
    pub allow_acr_collection: Option<bool>,
    pub allow_app_content_ad_targeting: Option<bool>,
    pub allow_business_analytics: Option<bool>,
    pub allow_camera_analytics: Option<bool>,
    pub allow_personalization: Option<bool>,
    pub allow_primary_browse_ad_targeting: Option<bool>,
    pub allow_primary_content_ad_targeting: Option<bool>,
    pub allow_product_analytics: Option<bool>,
    pub allow_remote_diagnostics: Option<bool>,
    pub allow_resume_points: Option<bool>,
    pub allow_unentitled_personalization: Option<bool>,
    pub allow_unentitled_resume_points: Option<bool>,
    pub allow_watch_history: Option<bool>,
    pub skip_restriction: Option<String>,
    pub video_dimensions: Option<Vec<i32>>,
    pub lifecycle_transition_validate: Option<bool>,
    pub media_progress_as_watched_events: Option<bool>,
    pub accessibility_audio_description_settings: Option<bool>,
    pub role_based_support: Option<bool>,
    pub country_postal_code: Option<HashMap<String, String>>,
    pub countries_using_us_privacy: Option<Vec<String>>,
}

impl MergeConfig<CascadedDefaultValues> for DefaultValues {
    fn merge_config(&mut self, cascaded: CascadedDefaultValues) {
        if let Some(cas_contry_code) = cascaded.country_code {
            self.country_code = cas_contry_code
        }
        if let Some(cas_language) = cascaded.language {
            self.language = cas_language
        }
        if let Some(cas_locale) = cascaded.locale {
            self.locale = cas_locale
        }
        if let Some(cas_name) = cascaded.name {
            self.name = cas_name
        }
        if let Some(cas_captions) = cascaded.captions {
            self.captions = cas_captions
        }
        if let Some(cas_additional_info) = cascaded.additional_info {
            self.additional_info.extend(cas_additional_info);
        }
        if let Some(cas_voice) = cascaded.voice {
            self.voice = cas_voice
        }
        if let Some(cas_allow_acr_collection) = cascaded.allow_acr_collection {
            self.allow_acr_collection = cas_allow_acr_collection
        }
        if let Some(cas_allow_app_ad_targetting) = cascaded.allow_app_content_ad_targeting {
            self.allow_app_content_ad_targeting = cas_allow_app_ad_targetting
        }
        if let Some(cas_allow_business_analytics) = cascaded.allow_business_analytics {
            self.allow_business_analytics = cas_allow_business_analytics
        }
        if let Some(cas_allow_camera_analytics) = cascaded.allow_camera_analytics {
            self.allow_camera_analytics = cas_allow_camera_analytics
        }
        if let Some(cas_allow_personalization) = cascaded.allow_personalization {
            self.allow_personalization = cas_allow_personalization
        }
        if let Some(cas_allow_primary_browse_ad_targeting) =
            cascaded.allow_primary_browse_ad_targeting
        {
            self.allow_primary_browse_ad_targeting = cas_allow_primary_browse_ad_targeting
        }
        if let Some(cas_allow_primary_content_ad_targeting) =
            cascaded.allow_primary_content_ad_targeting
        {
            self.allow_primary_content_ad_targeting = cas_allow_primary_content_ad_targeting
        }
        if let Some(cas_allow_product_analytics) = cascaded.allow_product_analytics {
            self.allow_product_analytics = cas_allow_product_analytics
        }
        if let Some(cas_allow_remote_diagnostics) = cascaded.allow_remote_diagnostics {
            self.allow_remote_diagnostics = cas_allow_remote_diagnostics
        }
        if let Some(cas_allow_resume_points) = cascaded.allow_resume_points {
            self.allow_resume_points = cas_allow_resume_points
        }

        if let Some(cas_allow_unentitled_personalization) = cascaded.allow_unentitled_personalization {
            self.allow_unentitled_personalization = cas_allow_unentitled_personalization;
        }
        if let Some(cas_allow_unentitled_resume_points) = cascaded.allow_unentitled_resume_points {
            self.allow_unentitled_resume_points = cas_allow_unentitled_resume_points;
        }
        if let Some(cas_allow_watch_history) = cascaded.allow_watch_history {
            self.allow_watch_history = cas_allow_watch_history;
        }
        if let Some(cas_skip_restriction) = cascaded.skip_restriction {
            self.skip_restriction = cas_skip_restriction;
        }
        if let Some(cas_video_dimentions) = cascaded.video_dimensions {
            self.video_dimensions.extend(cas_video_dimentions);
        }
        if let Some(cas_lifecycle_transition_validate) = cascaded.lifecycle_transition_validate {
            self.lifecycle_transition_validate = cas_lifecycle_transition_validate;
        }
        if let Some(cas_media_progress_as_watched_events) = cascaded.media_progress_as_watched_events {
            self.media_progress_as_watched_events = cas_media_progress_as_watched_events;
        }
        if let Some(cas_accessibility_audio_description_settings) = cascaded.accessibility_audio_description_settings {
            self.accessibility_audio_description_settings = cas_accessibility_audio_description_settings;
        }
        if let Some(cas_role_based_support) = cascaded.role_based_support {
            self.role_based_support = cas_role_based_support;
        }
        
        if let Some(cas_country_postal_code) = cascaded.country_postal_code {
            self.country_postal_code.extend(cas_country_postal_code);
        }
        if let Some(cas_countries_using_us_privacy) = cascaded.countries_using_us_privacy {
            self.countries_using_us_privacy
                .extend(cas_countries_using_us_privacy);
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CascadedExclusoryImpl {
    pub resolve_only: Option<Vec<String>>,
    pub app_authorization_rules: Option<CascadedAppAuthorizationRules>,
    /*
    method names to ignore regardless of appid
    */
    pub method_ignore_rules: Option<Vec<String>>,
}

impl MergeConfig<CascadedExclusoryImpl> for ExclusoryImpl {
    fn merge_config(&mut self, other: CascadedExclusoryImpl) {
        if let Some(other_resolve_only) = other.resolve_only {
            if self.resolve_only.is_none() {
                self.resolve_only = Some(other_resolve_only);
            } else {
                self.resolve_only
                    .as_mut()
                    .unwrap()
                    .extend(other_resolve_only);
            }
        }
        if let Some(other_rules) = other.app_authorization_rules {
            self.app_authorization_rules.merge_config(other_rules);
        }
        if let Some(other_method_ignore_rules) = other.method_ignore_rules {
            self.method_ignore_rules.extend(other_method_ignore_rules);
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct CascadedAppAuthorizationRules {
    pub app_ignore_rules: Option<HashMap<String, Vec<String>>>,
}

impl MergeConfig<CascadedAppAuthorizationRules> for AppAuthorizationRules {
    fn merge_config(&mut self, other: CascadedAppAuthorizationRules) {
        if let Some(other_rules) = other.app_ignore_rules {
            for (key, values) in other_rules {
                self.app_ignore_rules.entry(key).or_default().extend(values);
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct SettingsDefaults {
    pub postal_code: String,
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

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CloudService {
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CascadedRippleFeatures {
    pub privacy_settings_storage_type: Option<PrivacySettingsStorageType>,
    pub intent_validation: Option<IntentValidation>,
    pub cloud_permissions: Option<bool>,
    pub catalog_uninstalls_enabled: Option<CascadedFeatureFlag>,
}

impl MergeConfig<CascadedRippleFeatures> for RippleFeatures {
    fn merge_config(&mut self, cascaded: CascadedRippleFeatures) {
        if let Some(cas_privacy_settings_storage_type) = cascaded.privacy_settings_storage_type {
            self.privacy_settings_storage_type = cas_privacy_settings_storage_type
        }
        if let Some(cas_intent_validation) = cascaded.intent_validation {
            self.intent_validation = cas_intent_validation
        }
        if let Some(cas_cloud_permission) = cascaded.cloud_permissions {
            self.cloud_permissions = cas_cloud_permission
        }
        if let Some(cas_catalog_uninstalls_enabled) = cascaded.catalog_uninstalls_enabled {
            self.catalog_uninstalls_enabled
                .merge_config(cas_catalog_uninstalls_enabled);
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CascadedFeatureFlag {
    pub default: Option<bool>,
    pub remote_key: Option<String>,
}

impl MergeConfig<CascadedFeatureFlag> for FeatureFlag {
    fn merge_config(&mut self, cascaded: CascadedFeatureFlag) {
        if let Some(cas_default) = cascaded.default {
            self.default = cas_default
        }
        if let Some(cas_remote_key) = cascaded.remote_key {
            self.remote_key = Some(cas_remote_key)
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CascadedDataGovernanceConfig {
    pub policies: Option<Vec<CascadedDataGovernancePolicy>>,
}

impl MergeConfig<CascadedDataGovernanceConfig> for DataGovernanceConfig {
    fn merge_config(&mut self, other: CascadedDataGovernanceConfig) {
        if let Some(other_policies) = other.policies {
            for cascaded_policy in other_policies {
                // Try to find a matching existing policy based on a unique identifier
                if let Some(existing_policy) = self.policies.iter_mut().find(|policy| {
                    // Replace 'policy.data_type' with the actual unique identifier field
                    if let Some(cascaded_data_type) = &cascaded_policy.data_type {
                        &policy.data_type == cascaded_data_type
                    } else {
                        false // Cannot match if cascaded data_type is None
                    }
                }) {
                    // If a matching policy is found, merge the cascaded values into it
                    existing_policy.merge_config(cascaded_policy.clone());
                } else {
                    // If no matching policy is found, and the cascaded policy has a unique identifier,
                    // attempt to create a new DataGovernancePolicy and add it.
                    // You'll need to define how to create a DataGovernancePolicy from a
                    // CascadedDataGovernancePolicy (handling the Option fields).
                    if let Some(data_type) = cascaded_policy.data_type.clone() {
                        // Example of creating a new policy - adjust based on your fields
                        let new_policy = DataGovernancePolicy {
                            data_type,
                            setting_tags: cascaded_policy
                                .setting_tags
                                .clone()
                                .unwrap_or_default()
                                .into_iter()
                                .filter_map(|c_tag| {
                                    c_tag.setting.map(|setting| DataGovernanceSettingTag {
                                        setting,
                                        enforcement_value: c_tag
                                            .enforcement_value
                                            .unwrap_or_default(),
                                        tags: c_tag.tags.clone().unwrap_or_default(),
                                    })
                                })
                                .collect(),
                            drop_on_all_tags: cascaded_policy.drop_on_all_tags.unwrap_or_default(),
                        };
                        self.policies.push(new_policy);
                    }
                    // If the unique identifier in the cascaded policy is None, we can't reliably
                    // create a new policy, so we skip it.
                }
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CascadedDataGovernancePolicy {
    pub data_type: Option<DataEventType>,
    pub setting_tags: Option<Vec<CascadedDataGovernanceSettingTag>>,
    pub drop_on_all_tags: Option<bool>,
}

impl MergeConfig<CascadedDataGovernancePolicy> for DataGovernancePolicy {
    fn merge_config(&mut self, cascaded: CascadedDataGovernancePolicy) {
        if let Some(cas_data_type) = cascaded.data_type {
            self.data_type = cas_data_type
        }
        if let Some(cas_drop_on_all_tags) = cascaded.drop_on_all_tags {
            self.drop_on_all_tags = cas_drop_on_all_tags;
        }
        if let Some(cas_setting_tags) = cascaded.setting_tags {
            for cascaded_tag in cas_setting_tags {
                // Try to find a matching existing tag based on the setting
                if let Some(cascaded_setting) = &cascaded_tag.setting {
                    if let Some(existing_tag) = self
                        .setting_tags
                        .iter_mut()
                        .find(|tag| &tag.setting == cascaded_setting)
                    {
                        // If a matching tag is found, merge the cascaded values into it
                        existing_tag.merge_config(cascaded_tag.clone());
                    } else {
                        // If no matching tag is found, and the cascaded tag has a setting,
                        // create a new DataGovernanceSettingTag and add it.
                        if let Some(setting) = cascaded_tag.setting {
                            self.setting_tags.push(DataGovernanceSettingTag {
                                setting,
                                enforcement_value: cascaded_tag
                                    .enforcement_value
                                    .unwrap_or_default(),
                                tags: cascaded_tag.tags.unwrap_or_default(),
                            });
                        }
                        // If cascaded_tag.setting is None, we can't reliably merge or create a new one
                        // as 'setting' is mandatory in DataGovernanceSettingTag.
                    }
                }
                // If cascaded_tag.setting is None, we can't reliably merge or create a new one.
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CascadedDataGovernanceSettingTag {
    pub setting: Option<StorageProperty>,
    pub enforcement_value: Option<bool>,
    pub tags: Option<HashSet<String>>,
}

impl MergeConfig<CascadedDataGovernanceSettingTag> for DataGovernanceSettingTag {
    fn merge_config(&mut self, cascaded: CascadedDataGovernanceSettingTag) {
        if let Some(cas_stting) = cascaded.setting {
            self.setting = cas_stting
        }
        if let Some(cas_enforcement_value) = cascaded.enforcement_value {
            self.enforcement_value = cas_enforcement_value;
        }
        if let Some(cas_tags) = cascaded.tags {
            self.tags.extend(cas_tags);
        }
    }
}
