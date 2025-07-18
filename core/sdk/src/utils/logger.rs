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

use std::collections::HashMap;
use std::sync::RwLock;
use std::{str::FromStr, sync::atomic::AtomicU32};

pub static LOG_COUNTER: AtomicU32 = AtomicU32::new(1);

lazy_static::lazy_static! {
    pub static ref MODULE_LOG_LEVELS: RwLock<HashMap<String, log::LevelFilter>> = RwLock::new(HashMap::new());
}

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

pub fn init_and_configure_logger(
    version: &str,
    name: String,
    additional_modules: Option<Vec<(String, log::LevelFilter)>>,
) -> Result<(), fern::InitError> {
    let log_string: String = std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".into());
    println!("log level {}", log_string);
    let _version_string = version.to_string();
    let filter = log::LevelFilter::from_str(&log_string).unwrap_or(log::LevelFilter::Info);
    let (extracted_module_name, extracted_level_filter) = additional_modules
        .clone()
        .and_then(|modules| modules.into_iter().last())
        .unwrap_or(("no_module".to_string(), log::LevelFilter::Off));
    if let Some(am) = additional_modules {
        am.iter().for_each(|(module_name, level_filter)| {
            MODULE_LOG_LEVELS
                .write()
                .unwrap()
                .insert(module_name.clone(), *level_filter);
        });
    }

    println!(
        "additional module: {}, Level filter : {}",
        extracted_module_name, extracted_level_filter
    );
    fern::Dispatch::new()
        .format(move |out, message, record| {
            let _v = LOG_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            #[cfg(not(feature = "sysd"))]
            if _v % 100 == 0 {
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
            if _v % 100 == 0 {
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
        .level_for(extracted_module_name, extracted_level_filter)
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
