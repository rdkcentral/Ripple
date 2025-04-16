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

use log::info;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

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
        let cascaded_config = std::env::var("CASCADED_CONFIGURATION")
            .ok()
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);
        println!("CASCADED_CONFIGURATION {}", cascaded_config);
        let base_path = Self::try_load_base_path();
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
            println!("cascaded configuration is set to false");
            Ok((
                LoadExtnManifestStep::get_manifest(),
                LoadDeviceManifestStep::get_manifest(),
            ))
        } else {
            let config_loader = loader.get_config_loader();
            println!("cascaded configuration is set to true");
            Ok((
                config_loader.get_extn_manifest(),
                config_loader.get_device_manifest(),
            ))
        }
    }

    fn try_load_base_path() -> String {
        if cfg!(feature = "local_dev") {
            if let Ok(path) = std::env::var("RIPPLE_BASE_PATH") {
                info!("Loaded base path from env: {}", path);
                return path;
            }
            if let Ok(home) = std::env::var("HOME") {
                let path = format!("{}/.ripple/rdke", home);
                if Path::new(&path).is_dir() {
                    info!("Loaded base path from home: {}", path);
                    return path;
                }
            }
        } else if cfg!(test) {
            if let Ok(path) = std::env::var("RIPPLE_BASE_PATH") {
                info!("Loaded base path from env (test): {}", path);
                return path;
            }
        }
        let default_path = "/etc/ripple/rdke";
        default_path.to_string()
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

    fn load_and_merge_extn_manifests(
        &self,
        paths: &[String],
        default_path: Option<String>,
    ) -> ExtnManifest {
        let mut merged_manifest = ExtnManifest::default();

        // Load the default manifest first
        if let Some(default_path_str) = &default_path {
            println!(
                "Loading default extension manifest from: {}",
                default_path_str
            );
            match ExtnManifest::load(default_path_str.clone()) {
                Ok((_, manifest)) => merged_manifest = manifest,
                Err(e) => eprintln!("Error loading default extension manifest: {}", e),
            }
        } else {
            println!("No default extension manifest path provided.");
        }

        // Merge other manifests
        println!("Merging other extension manifests...");
        for path in paths {
            if let Some(default_path_str) = &default_path {
                if path == default_path_str {
                    continue; // Skip the default path as it's already loaded
                }
            }

            println!("Attempting to merge extension manifest from: {}", path);
            match CascadedExtnManifest::load(path.clone()) {
                Ok((_, cas_manifest)) => {
                    println!(
                        "Successfully loaded and merging extension manifest from: {}",
                        path
                    );
                    merged_manifest.merge_config(cas_manifest);
                }
                Err(e) => {
                    eprintln!(
                        "Error loading or merging extension manifest from {}: {:?}",
                        path, e
                    );
                }
            }
        }

        if let Ok(json_string) = serde_json::to_string_pretty(&merged_manifest) {
            println!("Merged Extension Manifest:\n{}", json_string);
        } else {
            eprintln!("Error serializing merged extension manifest to JSON for printing",);
        }

        merged_manifest
    }

    fn load_and_merge_device_manifests(
        &self,
        paths: &[String],
        default_path: Option<String>,
    ) -> DeviceManifest {
        let mut merged_manifest = DeviceManifest::default();

        // Load the default manifest first
        if let Some(default_path_str) = &default_path {
            println!("Loading default device manifest from: {}", default_path_str);
            match DeviceManifest::load(default_path_str.clone()) {
                Ok((_, manifest)) => merged_manifest = manifest,
                Err(e) => eprintln!("Error loading default device manifest: {}", e),
            }
        } else {
            println!("No default device manifest path provided.");
        }

        // Merge other manifests
        for path in paths {
            if let Some(default_path_str) = &default_path {
                if path == default_path_str {
                    continue; // Skip the default path as it's already loaded
                }
            }

            match CascadedDeviceManifest::load(path.clone()) {
                Ok((_, cas_manifest)) => {
                    println!(
                        "Successfully loaded and merging device manifest from: {}",
                        path
                    );
                    merged_manifest.merge_config(cas_manifest);
                }
                Err(e) => {
                    eprintln!(
                        "Error loading or merging device manifest from {}: {:?}",
                        path, e
                    );
                }
            }
        }

        if let Ok(json_string) = serde_json::to_string_pretty(&merged_manifest) {
            println!("Merged Device Manifest:\n{}", json_string);
        } else {
            eprintln!("Error serializing merged device manifest to JSON for printing",);
        }

        merged_manifest
    }

    fn get_manifest_paths(&self, is_extn: bool) -> (Option<String>, Vec<String>) {
        let mut paths = Vec::new();
        let manifest_key = if is_extn { "extn" } else { "manifest" };
        let default_path = if is_extn {
            self.manifest_config.default.extn.as_str()
        } else {
            self.manifest_config.default.device.as_str()
        };

        let default_path_resolved = if !default_path.is_empty() {
            Some(self.resolve_path(default_path))
        } else {
            None
        };

        let tags_map = self.manifest_config.tags.as_ref();
        let build_config = self.manifest_config.build.as_ref();
        let device_type = self.device_type.as_deref();
        let default_tag = self.manifest_config.default.tag.as_deref();
        let country_code = self.country_code.as_str();

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
        (default_path_resolved, paths)
    }

    fn load_cascaded_extn_manifest(&self) -> ExtnManifest {
        println!("Loading cascaded extension manifest");
        let (default_path, paths) = self.get_manifest_paths(true);
        self.load_and_merge_extn_manifests(&paths, default_path)
    }

    fn load_cascaded_device_manifest(&self) -> DeviceManifest {
        println!("Loading cascaded device manifest");
        let (default_path, paths) = self.get_manifest_paths(false);
        self.load_and_merge_device_manifests(&paths, default_path)
    }

    pub fn get_extn_manifest(&self) -> ExtnManifest {
        if self.cascaded_config {
            self.load_cascaded_extn_manifest()
        } else {
            panic!("get_extn_manifest called in non-cascaded mode after initialization");
        }
    }

    pub fn get_device_manifest(&self) -> DeviceManifest {
        if self.cascaded_config {
            self.load_cascaded_device_manifest()
        } else {
            panic!("get_device_manifest called in non-cascaded mode after initialization");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    use crate::api::manifest::ripple_manifest_loader::RippleManifestLoader;

    #[test]
    fn test_initialize_with_cascaded_config() {
        // Set up environment variables
        env::set_var("CASCADED_CONFIGURATION", "true");
        env::set_var("COUNTRY", "us");
        env::set_var("DEVICE_PLATFORM", "tv");
        env::set_var("RIPPLE_BASE_PATH", "../../../firebolt-devices/rdke");
        let result = RippleManifestLoader::initialize();
        assert!(result.is_ok(), "Failed to initialize RippleManifestLoader");

        let (extn_manifest, device_manifest) = result.unwrap();

        assert!(matches!(extn_manifest, ExtnManifest { .. }));
        assert!(matches!(device_manifest, DeviceManifest { .. }));
    }

    #[test]
    fn test_initialize_with_cascaded_config_false() {
        // Set up environment variables
        env::set_var("COUNTRY", "us");
        env::set_var("DEVICE_PLATFORM", "tv");
        env::set_var("RIPPLE_BASE_PATH", "../../../firebolt-devices/rdke");
        println!("Current working directory: {:?}", std::env::current_dir());
        env::set_var("EXTN_MANIFEST", "../../../mock/extn.json");
        env::set_var("DEVICE_MANIFEST", "../../../mock/manifest.json");
        let result = RippleManifestLoader::initialize();
        assert!(result.is_ok(), "Failed to initialize RippleManifestLoader");

        let (extn_manifest, device_manifest) = result.unwrap();

        assert!(matches!(extn_manifest, ExtnManifest { .. }));
        assert!(matches!(device_manifest, DeviceManifest { .. }));
    }
}
