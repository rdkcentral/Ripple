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
    log::error,
};

use crate::state::bootstrap_state::BootstrapState;

use super::{
    extn::{
        check_launcher_step::CheckLauncherStep, load_extn_metadata_step::LoadExtensionMetadataStep,
        load_extn_step::LoadExtensionsStep, load_session_step::LoadDistributorValuesStep,
        start_extn_channel_step::StartExtnChannelsStep,
    },
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
/// 3. [LoadExtensionMetadataStep] - Loads the Extn metadata from the So files
/// 4. [LoadExtensionsStep] - Loads the Extensions in to [crate::state::extn_state::ExtnState]
/// 5. [StartExtnChannelsStep] - Starts the Device channel extension
/// 6. [StartAppManagerStep] - Starts the App Manager and other supporting services
/// 7. [StartOtherBrokers] - Start Other brokers if they are setup in endpoints for rules
/// 8. [LoadDistributorValuesStep] - Loads the values from distributor like Session
/// 9. [CheckLauncherStep] - Checks the presence of launcher extension and starts default app
/// 10. [StartWsStep] - Starts the Websocket to accept external and internal connections
/// 11. [FireboltGatewayStep] - Starts the firebolt gateway and blocks the thread to keep it alive till interruption.

///
pub async fn boot(state: BootstrapState) -> RippleResponse {
    let bootstrap = Bootstrap::new(state);
    execute_step(StartCommunicationBroker, &bootstrap).await?;
    execute_step(SetupExtnClientStep, &bootstrap).await?;
    execute_step(LoadExtensionMetadataStep, &bootstrap).await?;
    execute_step(LoadExtensionsStep, &bootstrap).await?;
    execute_step(StartExtnChannelsStep, &bootstrap).await?;
    execute_step(StartAppManagerStep, &bootstrap).await?;
    execute_step(StartOtherBrokers, &bootstrap).await?;
    execute_step(LoadDistributorValuesStep, &bootstrap).await?;
    execute_step(CheckLauncherStep, &bootstrap).await?;
    execute_step(StartWsStep, &bootstrap).await?;
    execute_step(FireboltGatewayStep, &bootstrap).await?;
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
