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
    utils::{logger::init_and_configure_logger, test_utils::log_memory_usage},
};
use state::bootstrap_state::BootstrapState;
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
    // Init logger
    log_memory_usage("main:start");
    if let Err(e) = init_and_configure_logger(SEMVER_LIGHTWEIGHT, "gateway".into(), None) {
        println!("{:?} logger init error", e);
        return;
    }
    log_memory_usage("main:logging configured");
    info!("version {}", SEMVER_LIGHTWEIGHT);
    let bootstate = BootstrapState::build().expect("Failure to init state for bootstrap");
    log_memory_usage("main: bootstrap::build complete");

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
