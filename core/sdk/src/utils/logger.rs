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

use super::error::RippleError;
use crate::api::manifest::device_manifest::DeviceManifest;
use log::info;
use std::{str::FromStr, sync::atomic::AtomicU32};

pub static LOG_COUNTER: AtomicU32 = AtomicU32::new(1);

pub fn init_logger(name: String) -> Result<(), fern::InitError> {
    let log_string: String = std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".into());
    println!("log level {}", log_string);
    let filter = log::LevelFilter::from_str(&log_string).unwrap_or(log::LevelFilter::Info);
    fern::Dispatch::new()
        .format(move |out, message, record| {
            #[cfg(not(feature = "sysd"))]
            return out.finish(format_args!(
                "{}[{}][{}][{}]-{}",
                chrono::Local::now().format("%Y-%m-%d-%H:%M:%S.%3f"),
                record.level(),
                record.target(),
                name,
                message
            ));
            #[cfg(feature = "sysd")]
            return out.finish(format_args!(
                "[{}][{}][{}]-{}",
                record.level(),
                record.target(),
                name,
                message
            ));
        })
        .level(filter)
        //log filter applied here, making the log level to OFF for the below mentioned crates
        .level_for("h2", log::LevelFilter::Off)
        .level_for("hyper", log::LevelFilter::Off)
        .level_for("rustls", log::LevelFilter::Off)
        .level_for("tower", log::LevelFilter::Off)
        .level_for("tower_http", log::LevelFilter::Off)
        .level_for("jsonrpsee_client_transport", log::LevelFilter::Off)
        .level_for("jsonrpsee_core", log::LevelFilter::Off)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

pub fn init_and_configure_logger(version: &str, name: String) -> Result<(), fern::InitError> {
    let log_string: String = std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".into());
    println!("log level {}", log_string);
    let _version_string = version.to_string();
    let filter = log::LevelFilter::from_str(&log_string).unwrap_or(log::LevelFilter::Info);
    // Read enable_log_signal from manifest
    let enable_log_signal = read_enable_log_signal();
    fern::Dispatch::new()
        .format(move |out, message, record| {
            let _v = LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            #[cfg(not(feature = "sysd"))]
            if enable_log_signal && _v % 100 == 0 {
                LOG_COUNTER.store(1, std::sync::atomic::Ordering::Relaxed);
                return out.finish(format_args!(
                    "{}[{}][{}][{}][{}]-{}",
                    chrono::Local::now().format("%Y-%m-%d-%H:%M:%S.%3f"),
                    record.level(),
                    record.target(),
                    name,
                    _version_string,
                    message
                ));
            } else {
                return out.finish(format_args!(
                    "{}[{}][{}][{}]-{}",
                    chrono::Local::now().format("%Y-%m-%d-%H:%M:%S.%3f"),
                    record.level(),
                    record.target(),
                    name,
                    message
                ));
            }
            #[cfg(feature = "sysd")]
            if enable_log_signal && _v % 100 == 0 {
                LOG_COUNTER.store(1, std::sync::atomic::Ordering::Relaxed);
                return out.finish(format_args!(
                    "[{}][{}][{}][{}]-{}",
                    record.level(),
                    record.target(),
                    name,
                    _version_string,
                    message
                ));
            } else {
                return out.finish(format_args!(
                    "[{}][{}][{}]-{}",
                    record.level(),
                    record.target(),
                    name,
                    message
                ));
            }
        })
        .level(filter)
        //log filter applied here, making the log level to OFF for the below mentioned crates
        .level_for("h2", log::LevelFilter::Off)
        .level_for("hyper", log::LevelFilter::Off)
        .level_for("rustls", log::LevelFilter::Off)
        .level_for("tower", log::LevelFilter::Off)
        .level_for("tower_http", log::LevelFilter::Off)
        .level_for("jsonrpsee_client_transport", log::LevelFilter::Off)
        .level_for("jsonrpsee_core", log::LevelFilter::Off)
        .level_for("tokio_tungstenite", log::LevelFilter::Off)
        .level_for("tungstenite", log::LevelFilter::Off)
        .level_for("soketto", log::LevelFilter::Off)
        .level_for("tracing", log::LevelFilter::Off)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

type DeviceManifestLoader = Vec<fn() -> Result<(String, DeviceManifest), RippleError>>;

fn try_manifest_files() -> Result<DeviceManifest, RippleError> {
    let dm_arr: DeviceManifestLoader = if cfg!(feature = "local_dev") {
        vec![load_from_env, load_from_home]
    } else if cfg!(test) {
        vec![load_from_env]
    } else {
        vec![load_from_etc]
    };

    for dm_provider in dm_arr {
        if let Ok((p, m)) = dm_provider() {
            info!("loaded_manifest_file_content={}", p);
            return Ok(m);
        }
    }
    Err(RippleError::BootstrapError)
}

fn load_from_env() -> Result<(String, DeviceManifest), RippleError> {
    let device_manifest_path = std::env::var("DEVICE_MANIFEST");
    match device_manifest_path {
        Ok(path) => DeviceManifest::load(path),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_home() -> Result<(String, DeviceManifest), RippleError> {
    match std::env::var("HOME") {
        Ok(home) => DeviceManifest::load(format!("{}/.ripple/firebolt-device-manifest.json", home)),
        Err(_) => Err(RippleError::MissingInput),
    }
}

fn load_from_etc() -> Result<(String, DeviceManifest), RippleError> {
    DeviceManifest::load("/etc/firebolt-device-manifest.json".into())
}

fn read_enable_log_signal() -> bool {
    try_manifest_files()
        .ok()
        .and_then(|manifest| serde_json::to_value(&manifest.configuration).ok())
        .and_then(|value| {
            value
                .get("enable_log_signal")
                .and_then(|signal_value| signal_value.as_bool())
        })
        .unwrap_or_default()
}
