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

use std::os::raw::c_char;

// MEMORY FIX: Enable jemalloc with aggressive memory return to OS
// Testing showed jemalloc outperforms mimalloc for this workload (4× less growth rate)
#[repr(transparent)]
pub struct ConfPtr(*const c_char);
unsafe impl Sync for ConfPtr {}

// CRITICAL: Aggressive decay for steady-state memory (return memory to OS quickly)
// narenas:1 limits arena count to 2 total (1 explicit + automatic arena 0) for minimal fragmentation
// dirty_decay_ms:100 returns memory faster than 250ms (embedded platform optimization)
// muzzy_decay_ms:100 matches dirty decay for consistency
// lg_tcache_max:12 reduces thread cache from 16KB to 4KB per thread (2 worker threads = 8KB total)
// retain:false disables jemalloc's internal extent retention (forces OS return on decay)
static STEADY_STATE_CONFIG: &[u8] =
    b"narenas:1,background_thread:true,dirty_decay_ms:100,muzzy_decay_ms:100,lg_tcache_max:12,retain:false\0";

#[no_mangle]
#[used]
pub static malloc_conf: ConfPtr = ConfPtr(STEADY_STATE_CONFIG.as_ptr() as *const c_char);

use tikv_jemallocator::Jemalloc;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

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
