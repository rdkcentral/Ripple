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
