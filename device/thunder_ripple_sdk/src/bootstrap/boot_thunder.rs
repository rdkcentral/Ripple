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
use crate::client::thunder_client::ThunderClientBuilder;
use crate::thunder_state::ThunderBootstrapStateWithConfig;
use crate::thunder_state::ThunderState;

// pub async fn boot_thunder(
//     ext_client: ExtnClient,
//     plugin_param: ThunderPluginBootParam,
// ) -> Option<ThunderBootstrapStateWithClient> {
//     info!("Booting thunder");
//     if ext_client.get_bool_config("use_with_thunder_broker") {
//         info!("Using thunder broker");

//         if let Ok(thndr_client) = ThunderClientBuilder::get_client(None, None, None, None, None, true)
//             .await{
//                 let thunder_state = ThunderState::new(ext_client.clone(), thndr_client);
//             }

//         None
//     } else {
//         if let Ok(state) = ThunderGetConfigStep::setup(ext_client, plugin_param).await {
//             if let Ok(state) = ThunderPoolStep::setup(state).await {
//                 SetupThunderProcessor::setup(state.clone()).await;
//                 return Some(state);
//             } else {
//                 error!("Unable to connect to Thunder, error in ThunderPoolStep");
//             }
//         } else {
//             error!("Unable to connect to Thunder, error in ThunderGetConfigStep");
//         }
//         None
//     }
// }

use std::sync::Arc;

pub async fn boot_thunder(
    ext_client: ExtnClient,
    plugin_param: ThunderPluginBootParam,
) -> Option<ThunderBootstrapStateWithClient> {
    info!("Booting thunder");
    if ext_client.get_bool_config("use_with_thunder_broker") {
        info!("Using thunder broker");

        if let Ok(thndr_client) =
            ThunderClientBuilder::get_client(None, None, None, None, None, true).await
        {
            let thunder_state = ThunderState::new(ext_client.clone(), thndr_client);

            let thndr_boot_statecfg = ThunderBootstrapStateWithConfig {
                extn_client: ext_client,
                url: None,
                pool_size: None,
                plugin_param: None,
                thunder_connection_state: None,
            };

            let thndr_boot_stateclient = ThunderBootstrapStateWithClient {
                prev: thndr_boot_statecfg,
                state: thunder_state,
            };
            return Some(thndr_boot_stateclient);
        }

        None
    } else {
        if let Ok(state) = ThunderGetConfigStep::setup(ext_client, plugin_param).await {
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
}
