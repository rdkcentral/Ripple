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
    client::plugin_manager::ThunderPluginBootParam, thunder_state::ThunderBootstrapStateWithClient,
};
use ripple_sdk::{
    extn::client::extn_client::ExtnClient,
    log::{error, info},
};

use super::{get_config_step::ThunderGetConfigStep, setup_thunder_pool_step::ThunderPoolStep};

pub async fn boot_thunder(
    state: ExtnClient,
    plugin_param: ThunderPluginBootParam,
) -> Option<ThunderBootstrapStateWithClient> {
    info!("Booting thunder");
    if let Ok(state) = ThunderGetConfigStep::setup(state, plugin_param).await {
        if let Ok(state) = ThunderPoolStep::setup(state).await {
            SetupThunderProcessor::setup(state.clone()).await;
            return Some(state);
        } else {
            error!("Unable to connect to Thunder, error in ThunderPoolStep");
        }
    } else {
        error!("Unable to connect to Thunder, error in ThunderGetConfigStep");
    }
    None
}
