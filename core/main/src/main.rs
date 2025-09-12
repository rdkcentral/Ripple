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

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[tokio::main(worker_threads = 2)]
async fn main() {
    let _profiler = {
        let profiler = dhat::Profiler::builder()
            .file_name("/tmp/ripple-heap.json")
            .build();

        // Signal handling for on-demand status info and shutdown
        let profiler_for_shutdown = std::sync::Arc::new(std::sync::Mutex::new(Some(profiler)));
        let profiler_clone = profiler_for_shutdown.clone();

        std::thread::spawn(move || {
            use signal_hook::{
                consts::{SIGINT, SIGTERM, SIGUSR1},
                iterator::Signals,
            };

            let mut signals = Signals::new([SIGUSR1, SIGTERM, SIGINT])
                .expect("Failed to register signal handler");

            for sig in signals.forever() {
                match sig {
                    SIGUSR1 => {
                        eprintln!(
                            "dhat: SIGUSR1 received - profiler is active and collecting data"
                        );
                        eprintln!("dhat: Heap profile will be written to /opt/ripple-heap.json when process exits");
                        eprintln!("dhat: Current PID: {}", std::process::id());

                        // Print some basic memory stats
                        let stats = dhat::HeapStats::get();
                        eprintln!("dhat: Current stats - Total blocks: {}, Total bytes: {}, Max blocks: {}, Max bytes: {}", 
                                stats.total_blocks, stats.total_bytes, stats.max_blocks, stats.max_bytes);
                    }
                    SIGTERM | SIGINT => {
                        eprintln!(
                            "dhat: Received shutdown signal ({}), dropping profiler...",
                            if sig == SIGTERM { "SIGTERM" } else { "SIGINT" }
                        );

                        // Drop the profiler to write the file before exit
                        if let Ok(mut guard) = profiler_clone.lock() {
                            if let Some(prof) = guard.take() {
                                drop(prof);
                                eprintln!("dhat: Heap profile written to /tmp/ripple-heap.json");
                            }
                        }

                        // Exit the process
                        std::process::exit(if sig == SIGTERM { exitcode::OK } else { 130 });
                        // 130 is typical for SIGINT
                    }
                    _ => {
                        eprintln!("dhat: Received unexpected signal: {}", sig);
                    }
                }
            }
        });

        eprintln!("dhat: Memory profiling enabled. Profile will be saved to /opt/ripple-heap.json on exit");
        eprintln!(
            "dhat: Send SIGUSR1 for current stats: kill -USR1 {}",
            std::process::id()
        );
        eprintln!("dhat: Send SIGTERM/SIGINT for graceful shutdown with profile dump");

        profiler_for_shutdown
    };

    // Init logger
    if let Err(e) = init_and_configure_logger(SEMVER_LIGHTWEIGHT, "gateway".into(), None) {
        println!("{:?} logger init error", e);
        eprintln!("dhat: Dropping profiler due to logger error...");
        if let Ok(mut guard) = _profiler.lock() {
            if let Some(prof) = guard.take() {
                drop(prof);
                eprintln!("dhat: Heap profile written to /opt/ripple-heap.json");
            }
        }
        return;
    }
    info!("version {}", SEMVER_LIGHTWEIGHT);
    let bootstate = BootstrapState::build().expect("Failure to init state for bootstrap");

    // bootstrap
    match boot(bootstate).await {
        Ok(_) => {
            info!("Ripple Exited gracefully!");
            eprintln!("dhat: Dropping profiler to write heap profile...");
            if let Ok(mut guard) = _profiler.lock() {
                if let Some(prof) = guard.take() {
                    drop(prof);
                    eprintln!("dhat: Heap profile written to /opt/ripple-heap.json");
                }
            }
            std::process::exit(exitcode::OK);
        }
        Err(e) => {
            error!("Ripple failed with Error: {:?}", e);
            eprintln!("dhat: Dropping profiler to write heap profile...");
            if let Ok(mut guard) = _profiler.lock() {
                if let Some(prof) = guard.take() {
                    drop(prof);
                    eprintln!("dhat: Heap profile written to /opt/ripple-heap.json");
                }
            }
            std::process::exit(exitcode::SOFTWARE);
        }
    }
}
