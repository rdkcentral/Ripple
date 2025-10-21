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
pub mod bootstrap;
pub mod broker;
pub mod firebolt;
pub mod processor;
pub mod service;
pub mod state;
pub mod utils;
include!(concat!(env!("OUT_DIR"), "/version.rs"));

// use std::os::raw::c_char;

// // 1) Wrap the pointer so we can mark it Sync.
// #[repr(transparent)]
// pub struct ConfPtr(*const c_char);
// unsafe impl Sync for ConfPtr {} // ok: points to immutable static bytes

// // 2) The actual bytes (null-terminated). These are immutable and live forever.
// //static MALLOC_CONF_BYTES: &[u8] = b"background_thread:true,dirty_decay_ms:1000,\
// //muzzy_decay_ms:1000,percpu_arena:disabled,narenas:4,metadata_thp:never,lg_tcache_max:16\0";
// static SQUEEZE_HARD: &[u8] = b"background_thread:true,dirty_decay_ms:500,muzzy_decay_ms:500,percpu_arena:disabled,narenas:1,metadata_thp:never,lg_tcache_max:16\0";

// // 3) Export the symbol jemalloc looks for. Keep it from being stripped.
// #[no_mangle]
// #[used]
// pub static malloc_conf: ConfPtr = ConfPtr(SQUEEZE_HARD.as_ptr() as *const c_char);

// use tikv_jemallocator::Jemalloc;

// #[global_allocator]
// static GLOBAL: Jemalloc = Jemalloc;

#[tokio::main(worker_threads = 2)]
async fn main() {
    // Init logger
    if let Err(e) = init_and_configure_logger(SEMVER_LIGHTWEIGHT, "gateway".into(), None) {
        println!("{:?} logger init error", e);
        return;
    }
    info!("version {}", SEMVER_LIGHTWEIGHT);
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
