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
            .file_name("/opt/ripple-heap.json")
            .build();
        
        // Create a background thread that periodically creates snapshots
        std::thread::spawn(|| {
            let mut counter = 0;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(300)); // Every 5 minutes
                counter += 1;
                
                // Create a timestamp-based snapshot
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let snapshot_file = format!("/opt/ripple-heap-snapshot-{}-{}.json", timestamp, counter);
                
                eprintln!("dhat: Creating periodic snapshot: {}", snapshot_file);
                
                // Create a new profiler instance for snapshot
                let _snapshot_profiler = dhat::Profiler::builder()
                    .file_name(&snapshot_file)
                    .build();
                
                // Drop it immediately to write the file
                drop(_snapshot_profiler);
                eprintln!("dhat: Snapshot created: {}", snapshot_file);
            }
        });

        // Signal handling for on-demand dumps
        std::thread::spawn(|| {
            use signal_hook::{consts::SIGUSR1, iterator::Signals};
            
            let mut signals = Signals::new([SIGUSR1]).expect("Failed to register signal handler");
            
            for sig in signals.forever() {
                match sig {
                    SIGUSR1 => {
                        eprintln!("dhat: SIGUSR1 received, creating manual dump...");
                        
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let manual_dump = format!("/opt/ripple-heap-manual-{}.json", timestamp);
                        
                        // Create a temporary profiler for manual dump
                        let _dump_profiler = dhat::Profiler::builder()
                            .file_name(&manual_dump)
                            .build();
                        drop(_dump_profiler);
                        
                        eprintln!("dhat: Manual dump created: {}", manual_dump);
                    }
                    _ => {
                        eprintln!("dhat: Received unexpected signal: {}", sig);
                    }
                }
            }
        });

        eprintln!("dhat: Memory profiling enabled. Send SIGUSR1 for manual dump: kill -USR1 {}", std::process::id());
        
        profiler
    };

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
