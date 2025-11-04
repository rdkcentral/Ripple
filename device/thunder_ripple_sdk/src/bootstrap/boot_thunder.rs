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
    thunder_state::ThunderBootstrapStateWithClient,
};
use ripple_sdk::{
    extn::client::extn_client::ExtnClient,
    log::{debug, error, info, warn},
    serde_json,
};
use serde_json::Value;

use crate::client::thunder_client::ThunderClientBuilder;
use crate::thunder_state::ThunderBootstrapStateWithConfig;
use crate::thunder_state::ThunderState;
use serde::Deserialize;

const GATEWAY_DEFAULT: &str = "ws://127.0.0.1:9998/jsonrpc";

#[derive(Deserialize, Clone)]
pub struct ThunderPlatformParams {
    #[serde(default = "gateway_default")]
    gateway: String,
}

fn gateway_default() -> String {
    String::from(GATEWAY_DEFAULT)
}

pub async fn boot_thunder(
    ext_client: ExtnClient,
    thunder_parameters: &Value,
) -> Option<ThunderBootstrapStateWithClient> {
    info!("Booting thunder initiated, Using thunder_async_clinet");
    let mut status_check = true;

    if let Some(status) = ext_client.get_config("thunder_plugin_status_check_at_broker_start_up") {
        if let Ok(s) = status.parse() {
            status_check = s;
        }
    };

    let mut gateway_url = match url::Url::parse(GATEWAY_DEFAULT) {
        Ok(url) => url,
        Err(e) => {
            error!(
                "Could not parse default gateway URL '{}': {}",
                GATEWAY_DEFAULT, e
            );
            return None;
        }
    };
    serde_json::from_value(thunder_parameters.clone())
        .map(|thunder_parameters: ThunderPlatformParams| {
            url::Url::parse(&thunder_parameters.gateway).map_or_else(
                |_| {
                    warn!(
                        "Could not parse thunder gateway '{}', using default {}",
                        thunder_parameters.gateway, GATEWAY_DEFAULT
                    );
                },
                |gtway_url| {
                    debug!("Got url from device manifest");
                    gateway_url = gtway_url;
                },
            );
        })
        .unwrap_or_else(|_| {
            warn!(
                "Could not read thunder platform parameters, using default {}",
                GATEWAY_DEFAULT
            );
        });

    if let Ok(host_override) = std::env::var("DEVICE_HOST") {
        gateway_url.set_host(Some(&host_override)).ok();
    }

    let state = if let Ok(thndr_client) =
        ThunderClientBuilder::start_thunder_client(gateway_url.clone(), status_check).await
    {
        let thunder_state = ThunderState::new(ext_client.clone(), thndr_client);

        let thndr_boot_statecfg = ThunderBootstrapStateWithConfig {
            extn_client: ext_client,
            url: gateway_url,
            thunder_connection_state: None,
        };

        let thndr_boot_stateclient = ThunderBootstrapStateWithClient {
            prev: thndr_boot_statecfg,
            state: thunder_state,
        };

        thndr_boot_stateclient.clone().state.start_event_thread();

        Some(thndr_boot_stateclient)
    } else {
        error!("Unable to connect to Thunder, error in ThunderGetConfigStep");
        None
    };

    if let Some(s) = state.clone() {
        SetupThunderProcessor::setup(s).await;
    }
    state
}
