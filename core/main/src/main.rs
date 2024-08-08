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

use crate::bootstrap::boot::boot;
use ripple_sdk::{
    log::{error, info},
    tokio,
    utils::logger::init_and_configure_logger,
};
use state::bootstrap_state::BootstrapState;
use std::env;
pub mod bootstrap;
pub mod broker;
pub mod firebolt;
pub mod processor;
pub mod service;
pub mod state;
pub mod utils;
include!(concat!(env!("OUT_DIR"), "/version.rs"));

#[tokio::main(worker_threads = 2)]
async fn main() {
    // Check for --version flag
    let args: Vec<String> = env::args().collect();
    if args.contains(&"--version".to_string()) {
        println!("Sakshi ripple {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // Init logger
    if let Err(e) = init_and_configure_logger(SEMVER_LIGHTWEIGHT, "gateway".into()) {
        println!("{:?} logger init error", e);
        return;
    }
    info!("version {}", env!("CARGO_PKG_VERSION"));
    let bootstate = BootstrapState::build().expect("Failure to init state for bootstrap");

    // bootstrap
    match boot(bootstate).await {
        Ok(_) => {
            info!("Ripple Exited gracefully!");
            std::process::exit(exitcode::OK);
        }
        Err(e) => {
            error!("Ripple failed with Error: {:?}", e);
            std::process::exit(exitcode::SOFTWARE);
        }
    }
}
