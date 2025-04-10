use std::path::Path;

use log::info;
use serde::Deserialize;

use crate::api::manifest::cascaded_extn_manifest::CascadedExtnManifest;
use crate::api::manifest::MergeConfig;
use crate::api::manifest::{device_manifest::DeviceManifest, extn_manifest::ExtnManifest};
use crate::manifest::{device::LoadDeviceManifestStep, extn::LoadExtnManifestStep};
use crate::utils::error::RippleError;

#[derive(Clone, Debug, Deserialize)]
pub struct RippleManifestConfig {
    default: DefaultManifestConfig,
    tags: Option<std::collections::HashMap<String, ManifestEntry>>,
    build: Option<BuildConfig>,
}

#[derive(Clone, Debug, Deserialize)]
struct DefaultManifestConfig {
    device: String,
    extn: String,
    tag: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct ManifestEntry {
    manifest: Option<String>,
    extn: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct BuildConfig {
    country: Option<std::collections::HashMap<String, CountryConfig>>,
}

#[derive(Clone, Debug, Deserialize)]
struct CountryConfig {
    tags: Option<Vec<String>>,
}

pub struct RippleManifestLoader {
    cascaded_config: bool,
    manifest_config: Option<RippleManifestConfig>,
}

impl RippleManifestLoader {
    pub fn initialize() -> Result<(ExtnManifest, DeviceManifest), RippleError> {
        let cascaded_config = std::env::var("CASCADED_CONFIGURATION").is_ok();
        let base_dir = Some("/etc/ripple/rdke".to_string());
        let manifest_config = if cascaded_config {
            match std::fs::read_to_string("/etc/ripple/rdke/ripple.config.json") {
                Ok(content) => match serde_json::from_str::<RippleManifestConfig>(&content) {
                    Ok(config) => Some(config),
                    Err(e) => {
                        eprintln!("Error deserializing ripple.config.json: {}", e);
                        None
                    }
                },
                Err(e) => {
                    eprintln!("Error reading ripple.config.json: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let loader = RippleManifestLoader {
            cascaded_config,
            manifest_config,
        };
        let config_loader = loader.get_config_loader(base_dir);
        let extn_manifest = config_loader.get_extn_manifest();
        let device_manifest = config_loader.get_device_manifest();

        Ok((extn_manifest, device_manifest))
    }

    pub fn get_config_loader(&self, base_path: Option<String>) -> RippleConfigLoader {
        RippleConfigLoader {
            cascaded_config: self.cascaded_config,
            manifest_config: self
                .manifest_config
                .clone()
                .unwrap_or(RippleManifestConfig {
                    default: DefaultManifestConfig {
                        device: String::new(),
                        extn: String::new(),
                        tag: None,
                    },
                    tags: None,
                    build: None,
                }),
            base_path,
        }
    }
}

pub struct RippleConfigLoader {
    cascaded_config: bool,
    manifest_config: RippleManifestConfig,
    base_path: Option<String>,
}

impl RippleConfigLoader {
    pub fn get_extn_manifest(&self) -> ExtnManifest {
        let mut merged_extn_manifest = ExtnManifest::default();

        if self.cascaded_config {
            println!("Loading cascaded extension manifest");
            let extn_paths = self.get_all_extn_manifest_paths();
            println!("Extension Manifest Paths to Merge: {:?}", extn_paths);
            for path in extn_paths {
                match self.load_cascaded_extn_manifest(&path) {
                    Ok(cascaded_manifest) => {
                        merged_extn_manifest.merge_config(cascaded_manifest);
                    }
                    Err(e) => {
                        eprintln!(
                            "Error loading or merging extension manifest from {}: {:?}",
                            path, e
                        );
                    }
                }
            }
            // Serialize and print the merged extension manifest to JSON
            match serde_json::to_string_pretty(&merged_extn_manifest) {
                Ok(json_string) => {
                    println!("Merged Extension Manifest:\n{}", json_string);
                }
                Err(e) => {
                    eprintln!("Error serializing merged extension manifest to JSON: {}", e);
                }
            }
            merged_extn_manifest
        } else {
            println!("Loading default extn manifest (non-cascaded)");
            LoadExtnManifestStep::get_manifest()
        }
    }

    fn load_cascaded_extn_manifest(&self, path: &str) -> Result<CascadedExtnManifest, RippleError> {
        match CascadedExtnManifest::load(path.into()) {
            Ok((loaded_path, cascaded_manifest)) => {
                info!("loaded_cascaded_extn_file_content={}", loaded_path);
                Ok(cascaded_manifest)
            }
            Err(err) => {
                eprintln!(
                    "Error loading cascaded extension manifest from {}: {:?}",
                    path, err
                );
                Err(err)
            }
        }
    }
    pub fn get_device_manifest(&self) -> DeviceManifest {
        if self.cascaded_config {
            println!("Loading cascaded device manifest");
            let device_paths = self.get_all_device_manifest_paths();
            println!("Device Manifest Paths to Merge: {:?}", device_paths);
            // TODO: Implement logic to load device manifest from these paths
            LoadDeviceManifestStep::get_manifest()
        } else {
            println!("Loading default device manifest (non-cascaded)");
            LoadDeviceManifestStep::get_manifest()
        }
    }

    fn get_all_device_manifest_paths(&self) -> Vec<String> {
        let mut device_paths = Vec::new();
        let base_path = self.base_path.as_deref();
        let default_device = &self.manifest_config.default.device;
        let tags_map = self.manifest_config.tags.as_ref();
        let build_config = self.manifest_config.build.as_ref();
        let country = std::env::var("COUNTRY").ok();
        let device_type = std::env::var("DEVICE_PLATFORM").ok();
        let default_tag = self.manifest_config.default.tag.as_deref();

        // Helper function to resolve path with base
        let resolve_path = |path: &str| {
            if let Some(base) = base_path {
                if !path.is_empty() && path.starts_with("/") {
                    Path::new(base)
                        .join(&path[1..])
                        .to_string_lossy()
                        .into_owned()
                } else {
                    path.to_string()
                }
            } else {
                path.to_string()
            }
        };
        println!("device_paths--{:?}", device_paths);
        // 1. Default device path
        if !default_device.is_empty() {
            device_paths.push(resolve_path(default_device));
        }
        println!("device_paths--{:?}", device_paths);

        let mut tags_to_process = Vec::new();
        if let Some(country_code) = country {
            if let Some(build) = build_config {
                if let Some(country_map) = &build.country {
                    if let Some(country_config) = country_map.get(&country_code) {
                        if let Some(tags) = &country_config.tags {
                            tags_to_process.extend(tags.iter().map(|s| s.as_str()));
                        }
                    }
                }
            }
        } else if let Some(tag) = default_tag {
            tags_to_process.push(tag);
        }

        // Process paths from tags
        for tag in tags_to_process.iter() {
            if let Some(tags) = tags_map {
                if let Some(entry) = tags.get(*tag) {
                    if let Some(manifest_path) = &entry.manifest {
                        device_paths.push(resolve_path(manifest_path));
                    }
                }
            }
            // Process with DEVICE_PLATFORM
            if let Some(dt) = &device_type {
                let combined_tag = format!("{}_{}", tag, dt);
                if let Some(tags) = tags_map {
                    if let Some(entry) = tags.get(&combined_tag) {
                        if let Some(manifest_path) = &entry.manifest {
                            device_paths.push(resolve_path(manifest_path));
                        }
                    }
                }
            }
        }
        device_paths
    }

    fn get_all_extn_manifest_paths(&self) -> Vec<String> {
        let mut extn_paths = Vec::new();
        let base_path = self.base_path.as_deref();
        let default_extn = &self.manifest_config.default.extn;
        let tags_map = self.manifest_config.tags.as_ref();
        let build_config = self.manifest_config.build.as_ref();
        let country = std::env::var("COUNTRY").ok();
        let device_type = std::env::var("DEVICE_PLATFORM").ok();
        let default_tag = self.manifest_config.default.tag.as_deref();

        // Helper function to resolve path with base
        let resolve_path = |path: &str| {
            if let Some(base) = base_path {
                if !path.is_empty() && path.starts_with("/") {
                    Path::new(base)
                        .join(&path[1..])
                        .to_string_lossy()
                        .into_owned()
                } else {
                    path.to_string()
                }
            } else {
                path.to_string()
            }
        };

        // 1. Default extension path
        if !default_extn.is_empty() {
            extn_paths.push(resolve_path(default_extn));
        }

        let mut tags_to_process = Vec::new();
        if let Some(country_code) = country {
            if let Some(build) = build_config {
                if let Some(country_map) = &build.country {
                    if let Some(country_config) = country_map.get(&country_code) {
                        if let Some(tags) = &country_config.tags {
                            tags_to_process.extend(tags.iter().map(|s| s.as_str()));
                        }
                    }
                }
            }
        } else if let Some(tag) = default_tag {
            tags_to_process.push(tag);
        }

        // Process paths from tags
        for tag in tags_to_process.iter() {
            if let Some(tags) = tags_map {
                if let Some(entry) = tags.get(*tag) {
                    if let Some(extn_path) = &entry.extn {
                        extn_paths.push(resolve_path(extn_path));
                    }
                }
            }
            // Process with DEVICE_PLATFORM
            if let Some(dt) = &device_type {
                let combined_tag = format!("{}_{}", tag, dt);
                if let Some(tags) = tags_map {
                    if let Some(entry) = tags.get(&combined_tag) {
                        if let Some(extn_path) = &entry.extn {
                            extn_paths.push(resolve_path(extn_path));
                        }
                    }
                }
            }
        }
        extn_paths
    }
}
