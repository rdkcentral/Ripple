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
    api::status_update::ExtnStatus,
    log::{info, warn},
    utils::error::RippleError,
};

use crate::{
    client::{plugin_manager::PluginManager, thunder_client_pool::ThunderClientPool},
    thunder_state::{
        ThunderBootstrapStateWithClient, ThunderBootstrapStateWithConfig, ThunderState,
    },
};

pub struct ThunderPoolStep;

impl ThunderPoolStep {
    pub fn get_name() -> String {
        "ThunderPoolStep".into()
    }

    pub async fn setup(
        state: ThunderBootstrapStateWithConfig,
    ) -> Result<ThunderBootstrapStateWithClient, RippleError> {
        let pool_size = state.pool_size;
        let url = state.url.clone();
        let thunder_connection_state = state.thunder_connection_state.clone();
        if pool_size < 2 {
            warn!("Pool size of 1 is not recommended, there will be no dedicated connection for Controller events");
            return Err(RippleError::BootstrapError);
        }
        let controller_pool =
            ThunderClientPool::start(url.clone(), None, thunder_connection_state.clone(), 1).await;
        if controller_pool.is_err() {
            if let Err(e) = controller_pool {
                let _ = state.extn_client.event(ExtnStatus::Error);
                return Err(e);
            }
        }
        info!("Received Controller pool");
        let controller_pool = controller_pool.unwrap();
        let expected_plugins = state.plugin_param.clone();
        let plugin_manager_tx =
            PluginManager::start(Box::new(controller_pool), expected_plugins).await;
        let client = ThunderClientPool::start(
            url,
            Some(plugin_manager_tx),
            thunder_connection_state.clone(),
            pool_size - 1,
        )
        .await;

        if client.is_err() {
            if let Err(e) = client {
                let _ = state.extn_client.event(ExtnStatus::Error);
                return Err(e);
            }
        }

        let client = client.unwrap();
        info!("Thunder client connected successfully");

        let extn_client = state.extn_client.clone();
        let thunder_boot_strap_state_with_client = ThunderBootstrapStateWithClient {
            prev: state,
            state: ThunderState::new(extn_client, client),
        };
        thunder_boot_strap_state_with_client
            .state
            .start_event_thread();
        Ok(thunder_boot_strap_state_with_client)
    }
}
