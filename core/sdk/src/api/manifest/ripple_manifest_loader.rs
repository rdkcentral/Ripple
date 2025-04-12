use std::collections::HashMap;
use std::path::Path;

use log::{info, warn};
use serde::Deserialize;
use serde::Serialize;

use crate::{
    api::manifest::{
        cascaded_device_manifest::CascadedDeviceManifest,
        cascaded_extn_manifest::CascadedExtnManifest, device_manifest::DeviceManifest,
        extn_manifest::ExtnManifest, MergeConfig,
    },
    manifest::{device::LoadDeviceManifestStep, extn::LoadExtnManifestStep},
    utils::error::RippleError,
};

#[derive(Clone, Debug, Deserialize, Default)]
pub struct RippleManifestConfig {
    default: DefaultManifestConfig,
    tags: Option<HashMap<String, ManifestEntry>>,
    build: Option<BuildConfig>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct DefaultManifestConfig {
    device: String,
    extn: String,
    tag: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct ManifestEntry {
    manifest: Option<String>,
    extn: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct BuildConfig {
    country: Option<HashMap<String, CountryConfig>>,
}

#[derive(Clone, Debug, Deserialize, Default)]
struct CountryConfig {
    tags: Option<Vec<String>>,
}

pub struct RippleManifestLoader {
    cascaded_config: bool,
    manifest_config: Option<RippleManifestConfig>,
    base_path: String,
    country_code: String,
    device_type: Option<String>,
}

impl RippleManifestLoader {
    pub fn initialize() -> Result<(ExtnManifest, DeviceManifest), RippleError> {
        let cascaded_config = std::env::var("CASCADED_CONFIGURATION").is_ok();
        let base_path = "/etc/ripple/rdke".to_string();
        let country_code = std::env::var("COUNTRY").unwrap_or_else(|_| "eu".to_string());
        let device_type = std::env::var("DEVICE_PLATFORM").ok();

        let manifest_config = if cascaded_config {
            Path::new(&base_path)
                .join("ripple.config.json")
                .to_str()
                .and_then(Self::load_ripple_config)
        } else {
            None
        };

        let loader = RippleManifestLoader {
            cascaded_config,
            manifest_config,
            base_path,
            country_code,
            device_type,
        };

        if !cascaded_config {
            Ok((
                LoadExtnManifestStep::get_manifest(),
                LoadDeviceManifestStep::get_manifest(),
            ))
        } else {
            let config_loader = loader.get_config_loader();
            Ok((
                config_loader.get_extn_manifest(),
                config_loader.get_device_manifest(),
            ))
        }
    }

    fn load_ripple_config(path: &str) -> Option<RippleManifestConfig> {
        std::fs::read_to_string(path)
            .map_err(|e| eprintln!("Error reading config from {}: {}", path, e))
            .ok()
            .and_then(|content| {
                serde_json::from_str::<RippleManifestConfig>(&content)
                    .map_err(|e| eprintln!("Error deserializing config from {}: {}", path, e))
                    .ok()
            })
    }

    pub fn get_config_loader(&self) -> RippleConfigLoader {
        RippleConfigLoader {
            cascaded_config: self.cascaded_config,
            manifest_config: self.manifest_config.clone().unwrap_or_default(),
            base_path: self.base_path.clone(),
            country_code: self.country_code.clone(),
            device_type: self.device_type.clone(),
        }
    }
}

pub struct RippleConfigLoader {
    cascaded_config: bool,
    manifest_config: RippleManifestConfig,
    base_path: String,
    country_code: String,
    device_type: Option<String>,
}

impl RippleConfigLoader {
    fn resolve_path(&self, path: &str) -> String {
        if !path.is_empty() && path.starts_with("/") {
            Path::new(&self.base_path)
                .join(&path[1..])
                .to_string_lossy()
                .into_owned()
        } else {
            Path::new(&self.base_path)
                .join(path)
                .to_string_lossy()
                .into_owned()
        }
    }

    /*
        fn load_manifest<M: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<M, RippleError> {
            let path_owned = path.to_string();
            match M::load(path_owned) {
                Ok((_, manifest)) => Ok(manifest),
                Err(e) => Err(e),
            }
        }
    */
    fn load_manifest<M: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<M, RippleError> {
        let path_owned = path.to_string(); // Create an owned String
        std::fs::read_to_string(&path_owned)
            .map_err(move |_| RippleError::MissingInput)
            .and_then(move |content| {
                serde_json::from_str::<M>(&content).map_err(move |_| RippleError::InvalidInput)
            })
            .inspect(move |_| info!("Loaded manifest from: {}", path_owned))
            .inspect_err(move |e| warn!("{}", e))
    }
    fn load_and_merge_manifests<
        T: MergeConfig<C> + Default + Serialize + for<'de> Deserialize<'de>,
        C: for<'de> Deserialize<'de>,
    >(
        &self,
        paths: &[String],
        manifest_type: &str,
    ) -> T {
        let mut merged_manifest = T::default();
        println!("{} Manifest Paths to Merge: {:?}", manifest_type, paths);
        for path in paths {
            match self.load_manifest::<C>(path) {
                Ok(loaded_manifest) => {
                    merged_manifest.merge_config(loaded_manifest);
                }
                Err(e) => {
                    eprintln!(
                        "Error loading or merging {} manifest from {}: {:?}",
                        manifest_type, path, e
                    );
                }
            }
        }
        if let Ok(json_string) = serde_json::to_string_pretty(&merged_manifest) {
            println!("Merged {} Manifest:\n{}", manifest_type, json_string);
        } else {
            eprintln!(
                "Error serializing merged {} manifest to JSON for printing",
                manifest_type
            );
        }
        merged_manifest
    }

    fn get_manifest_paths(&self, is_extn: bool) -> Vec<String> {
        let mut paths = Vec::new();
        let manifest_key = if is_extn { "extn" } else { "manifest" };
        let default_path = if is_extn {
            self.manifest_config.default.extn.as_str()
        } else {
            self.manifest_config.default.device.as_str()
        };
        let tags_map = self.manifest_config.tags.as_ref();
        let build_config = self.manifest_config.build.as_ref();
        let device_type = self.device_type.as_deref();
        let default_tag = self.manifest_config.default.tag.as_deref();
        let country_code = self.country_code.as_str();

        if !default_path.is_empty() {
            paths.push(self.resolve_path(default_path));
        }

        let mut tags_to_process = Vec::new();
        let mut country_match_found = false; // Flag to track if a country match occurred
        if let Some(build) = build_config {
            if let Some(country_map) = &build.country {
                if let Some(country_config) = country_map.get(country_code) {
                    if let Some(tags) = &country_config.tags {
                        tags_to_process.extend(tags.iter().map(|s| s.as_str()));
                        country_match_found = true; // Set the flag to true HERE
                    }
                }
            }
        }
        // Only process default tag if no country-specific tags were found
        if !country_match_found {
            if let Some(tag) = default_tag {
                tags_to_process.push(tag);
            }
        }

        for tag in tags_to_process.iter() {
            if let Some(tags) = tags_map {
                if let Some(entry) = tags.get(*tag) {
                    if let Some(path) = match manifest_key {
                        "extn" => entry.extn.as_deref(),
                        "manifest" => entry.manifest.as_deref(),
                        _ => None,
                    } {
                        paths.push(self.resolve_path(path));
                    }
                }
                if let Some(dt) = &device_type {
                    let combined_tag = format!("{}-{}", tag, dt);
                    if let Some(entry) = tags.get(&combined_tag) {
                        if let Some(path) = match manifest_key {
                            "extn" => entry.extn.as_deref(),
                            "manifest" => entry.manifest.as_deref(),
                            _ => None,
                        } {
                            paths.push(self.resolve_path(path));
                        }
                    }
                }
            }
        }

        paths.dedup();
        paths
    }

    fn load_cascaded_manifest<
        M: MergeConfig<C> + Default + Serialize + for<'de> Deserialize<'de>,
        C: for<'de> Deserialize<'de>,
    >(
        &self,
        is_extn: bool,
        manifest_type: &str,
    ) -> M {
        println!("Loading cascaded {} manifest", manifest_type.to_lowercase());
        let mut paths = self.get_manifest_paths(is_extn);
        let default_path_ref = if is_extn {
            &self.manifest_config.default.extn
        } else {
            &self.manifest_config.default.device
        };
        let default_path = self.resolve_path(default_path_ref);
        if !default_path.is_empty() && !paths.contains(&default_path) {
            paths.insert(0, default_path);
        }
        self.load_and_merge_manifests::<M, C>(&paths, manifest_type)
    }

    pub fn get_extn_manifest(&self) -> ExtnManifest {
        if self.cascaded_config {
            self.load_cascaded_manifest::<ExtnManifest, CascadedExtnManifest>(true, "Extension")
        } else {
            println!("Loading default extn manifest (non-cascaded)");
            LoadExtnManifestStep::get_manifest()
        }
    }

    pub fn get_device_manifest(&self) -> DeviceManifest {
        if self.cascaded_config {
            self.load_cascaded_manifest::<DeviceManifest, CascadedDeviceManifest>(false, "Device")
        } else {
            println!("Loading default device manifest (non-cascaded)");
            LoadDeviceManifestStep::get_manifest()
        }
    }
}
