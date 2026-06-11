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

use ripple_sdk::{
    framework::{
        bootstrap::{Bootstep, Bootstrap},
        RippleResponse,
    },
    log::{debug, error},
    tokio,
};

use crate::state::bootstrap_state::BootstrapState;
use ripple_sdk::utils::test_utils::log_memory_usage;

use super::{
    extn::{load_extn_step::LoadExtensionsStep, load_session_step::LoadDistributorValuesStep},
    logging_bootstrap_step::LoggingBootstrapStep,
    setup_extn_client_step::SetupExtnClientStep,
    start_app_manager_step::StartAppManagerStep,
    start_communication_broker::{StartCommunicationBroker, StartOtherBrokers},
    start_fbgateway_step::FireboltGatewayStep,
    start_ws_step::StartWsStep,
};

/// Spawn a background task that periodically purges jemalloc arenas and flushes tokio caches
/// This is critical for embedded platforms where sustained traffic causes linear memory growth
/// Combined with retain:false config, this should force actual memory return to OS via munmap
fn spawn_periodic_memory_maintenance() {
    tokio::spawn(async {
        // 15-second interval balances aggressive memory return with minimal CPU overhead
        // For high-traffic scenarios (50+ ops/min), this prevents accumulation between purges
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        
        // Pre-allocate command buffers to avoid allocations in hot path
        // Max arena count is typically < 1000, so 32 bytes is sufficient
        let mut purge_cmd = String::with_capacity(32);
        let mut decay_cmd = String::with_capacity(32);
        
        loop {
            interval.tick().await;

            // Force jemalloc to purge dirty pages and decay them back to OS
            use tikv_jemalloc_ctl::{arenas, epoch, stats};

            // Update stats epoch to get current memory metrics
            if let Ok(e) = epoch::mib() {
                let _ = e.advance();
            }

            // Capture memory stats before purging
            let resident_before = stats::resident::read().unwrap_or(0);
            let mapped_before = stats::mapped::read().unwrap_or(0);

            // Purge all arenas AND force dirty/muzzy pages back to OS
            // With retain:false, this should trigger actual munmap() instead of just madvise()
            if let Ok(narenas) = arenas::narenas::read() {
                for arena_id in 0..narenas {
                    // First purge dirty pages to muzzy
                    purge_cmd.clear();
                    use std::fmt::Write;
                    let _ = write!(&mut purge_cmd, "arena.{}.purge\0", arena_id);
                    // SAFETY: purge_cmd is a valid null-terminated string conforming to jemalloc's
                    // mallctl interface. The arena_id is bounds-checked by the narenas loop.
                    // The operation is write-only (value=0) triggering a side effect to purge the arena.
                    // No safe wrapper exists in tikv-jemalloc-ctl for dynamic per-arena commands.
                    unsafe {
                        let _ = tikv_jemalloc_ctl::raw::write(purge_cmd.as_bytes(), 0usize);
                    }

                    // Then decay both dirty and muzzy pages immediately (forces memory return)
                    decay_cmd.clear();
                    let _ = write!(&mut decay_cmd, "arena.{}.decay\0", arena_id);
                    // SAFETY: Same invariants as purge above. Triggers immediate decay of both dirty
                    // and muzzy pages to force memory return to OS (munmap with retain:false config).
                    unsafe {
                        let _ = tikv_jemalloc_ctl::raw::write(decay_cmd.as_bytes(), 0usize);
                    }
                }

                // Update epoch again to capture post-purge stats
                if let Ok(e) = epoch::mib() {
                    let _ = e.advance();
                }

                // Measure memory freed by purge/decay cycle
                let resident_after = stats::resident::read().unwrap_or(0);
                let mapped_after = stats::mapped::read().unwrap_or(0);
                
                let resident_freed = resident_before.saturating_sub(resident_after);
                let mapped_freed = mapped_before.saturating_sub(mapped_after);
                
                debug!(
                    "Memory maintenance: purged {} arenas | freed: {} KB resident, {} KB mapped | resident: {} -> {} KB",
                    narenas,
                    resident_freed / 1024,
                    mapped_freed / 1024,
                    resident_before / 1024,
                    resident_after / 1024
                );
            }

            // Flush tokio worker thread allocator caches
            for _ in 0..5 {
                tokio::task::yield_now().await;
            }
        }
    });
}
/// Starts up Ripple uses `PlatformState` to manage State
/// # Arguments
/// * `platform_state` - PlatformState
///
/// # Panics
///
/// Bootstrap panics are fatal in nature and it could happen due to bad configuration or device state. Logs should provide more information on which step the failure had occurred.
///
/// # Steps
///
/// 1. [StartCommunicationBroker] - Initialize the communication broker to create Thunder broker if rules are setup.
/// 2. [SetupExtnClientStep] - Initializes the extn client to start the Inter process communication backbone
/// 4. [LoadExtensionsStep] - Loads the Extensions in to [crate::state::extn_state::ExtnState]
/// 6. [StartAppManagerStep] - Starts the App Manager and other supporting services
/// 7. [StartOtherBrokers] - Start Other brokers if they are setup in endpoints for rules
/// 8. [LoadDistributorValuesStep] - Loads the values from distributor like Session
/// 10. [StartWsStep] - Starts the Websocket to accept external and internal connections
/// 11. [FireboltGatewayStep] - Starts the firebolt gateway and blocks the thread to keep it alive till interruption.

///
pub async fn boot(state: BootstrapState) -> RippleResponse {
    log_memory_usage("boot-Begining");
    let bootstrap = Bootstrap::new(state);
    execute_step(LoggingBootstrapStep, &bootstrap).await?;
    log_memory_usage("After-LoggingBootstrapStep");

    // MEMORY FIX: Spawn periodic memory maintenance task for embedded platforms
    // On SOC, continuous app lifecycle traffic causes linear memory growth even with
    // tokio yielding. This task aggressively purges jemalloc arenas every 30s to
    // force memory return to OS during sustained traffic patterns.
    spawn_periodic_memory_maintenance();
    execute_step(StartWsStep, &bootstrap).await?;
    log_memory_usage("After-StartWsStep");
    execute_step(StartCommunicationBroker, &bootstrap).await?;
    log_memory_usage("After-StartCommunicationBroker");
    execute_step(SetupExtnClientStep, &bootstrap).await?;
    log_memory_usage("After-SetupExtnClientStep");
    let load_extensions = std::env::var("RIPPLE_RPC_EXTENSIONS")
        .ok()
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(true);
    if !load_extensions {
        debug!("Starting Ripple Service WITHOUT loading extension clients manifest");
    } else {
        debug!("Starting Ripple Service with extension clients");
        execute_step(LoadExtensionsStep, &bootstrap).await?;
    }
    log_memory_usage("After-LoadExtensionsStep");
    execute_step(StartAppManagerStep, &bootstrap).await?;
    log_memory_usage("After-StartAppManagerStep");
    execute_step(StartOtherBrokers, &bootstrap).await?;
    log_memory_usage("After-StartOtherBrokers");
    execute_step(LoadDistributorValuesStep, &bootstrap).await?;
    log_memory_usage("After-LoadDistributorValuesStep");
    execute_step(FireboltGatewayStep, &bootstrap).await?;
    log_memory_usage("After-FireboltGatewayStep");
    Ok(())
}

async fn execute_step<T: Bootstep<BootstrapState>>(
    step: T,
    state: &Bootstrap<BootstrapState>,
) -> RippleResponse {
    let name = step.get_name();
    if let Err(e) = state.step(step).await {
        error!("Failed at Bootstrap step {}", name);
        Err(e)
    } else {
        Ok(())
    }
}
