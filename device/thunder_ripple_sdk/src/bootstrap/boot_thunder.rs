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

use crate::{
    bootstrap::setup_thunder_processors::SetupThunderProcessor,
    client::plugin_manager::ThunderPluginBootParam,
    thunder_state::{ThunderBootstrapStateWithClient, ThunderBootstrapStateWithConfig},
};

use super::{get_config_step::ThunderGetConfigStep, setup_thunder_pool_step::ThunderPoolStep};
// Ensure the correct path to SetupThunderComcastProcessor
// use crate::client::thunder_client2;
use crate::thunder_state::ThunderState;
use ripple_sdk::{
    extn::client::extn_client::ExtnClient,
    log::{error, info},
};

pub async fn boot_thunder(
    client: ExtnClient,
    plugin_param: ThunderPluginBootParam,
) -> Option<ThunderBootstrapStateWithClient> {
    info!("Booting thunder");

    if client.get_bool_config("use_with_thunder_broker") {
        info!("thunderBroker_enabled feature is enabled");
        if let Ok(thndr_client) = thunder_client2::ThunderClientBuilder::get_client().await {
            let thunder_state = ThunderState::new(client.clone(), thndr_client);
            SetupThunderProcessor::setup(thunder_state.clone(), client.clone()).await;

            let thndr_st_config = ThunderBootstrapStateWithConfig {
                extn_client: client.clone(),
            };

            let thunder_bootstrap_state = ThunderBootstrapStateWithClient {
                prev: thndr_st_config,
                state: thunder_state,
            };
            return Some(thunder_bootstrap_state);
        } else {
            error!("Unable to connect to Thunder_Broker, error in ThunderClientBuilder");
        }
        None
    } else {
        info!("thunderBroker_enabled feature is not enabled, go for thunderclient");
        if let Ok(state) = ThunderGetConfigStep::setup(client, plugin_param).await {
            if let Ok(state) = ThunderPoolStep::setup(state).await {
                SetupThunderProcessor::setup(state.state.clone(), client.clone()).await;
                return Some(state);
            } else {
                error!("Unable to connect to Thunder, error in ThunderPoolStep");
            }
        } else {
            error!("Unable to connect to Thunder, error in ThunderGetConfigStep");
        };
        None
    }
}
