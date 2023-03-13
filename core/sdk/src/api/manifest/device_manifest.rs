use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fs};

use crate::{api::device::DevicePlatformType, utils::error::RippleError};

use super::{apps::AppManifest, exclusory::ExclusoryImpl};

#[derive(Deserialize, Debug, Clone)]
pub struct RippleConfiguration {
    pub ws_configuration: WsConfiguration,
    pub internal_ws_configuration: WsConfiguration,
    pub platform: DevicePlatformType,
    pub platform_parameters: Value,
    pub distribution_platform: String,
    pub distribution_tenant: String,
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
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantStep {
    pub capability: String,
    pub configuration: Option<Value>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantRequirements {
    pub steps: Vec<GrantStep>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Lifespan {
    Once,
    Forever,
    AppActive,
    PowerActive,
    Seconds,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum AutoApplyPolicy {
    Always,
    Allowed,
    Disallowed,
    Never,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySetting {
    pub property: String,
    pub auto_apply_policy: AutoApplyPolicy,
    pub update_property: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    App,
    Device,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantPolicy {
    pub options: GrantRequirements,
    pub scope: Scope,
    pub lifespan: Lifespan,
    pub overridable: bool,
    pub lifespan_ttl: Option<u32>,
    pub privacy_setting: Option<PrivacySetting>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GrantPolicies {
    #[serde(rename = "use")]
    pub _use: GrantPolicy,
    pub manage: GrantPolicy,
    pub provide: GrantPolicy,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityConifguration {
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
    pub capabilities: CapabilityConifguration,
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

#[derive(Deserialize, Debug, Clone)]
pub struct RippleFeatures {
    pub app_scoped_device_tokens: bool,
}

fn default_ripple_features() -> RippleFeatures {
    RippleFeatures {
        app_scoped_device_tokens: false,
    }
}

impl DeviceManifest {
    pub fn load(path: String) -> Result<(String, DeviceManifest), RippleError> {
        info!("Trying to load device manifest from path={}", path);
        if let Ok(contents) = fs::read_to_string(&path) {
            Self::load_from_content(contents)
        } else {
            info!("No device manifest found in {}", path);
            Err(RippleError::MissingInput)
        }
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
}
