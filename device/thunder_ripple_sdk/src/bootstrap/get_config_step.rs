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
    client::plugin_manager::ThunderPluginBootParam,
    thunder_state::{ThunderBootstrapStateWithConfig, ThunderConnectionState},
};
use std::sync::Arc;

use ripple_sdk::extn::{
    client::extn_client::ExtnClient,
    extn_client_message::{ExtnMessage, ExtnResponse},
};
use ripple_sdk::{
    api::config::Config,
    log::{debug, warn},
    serde_json::{self, Error},
    utils::error::RippleError,
};
use serde::Deserialize;
pub struct ThunderGetConfigStep;

const GATEWAY_DEFAULT: &str = "ws://127.0.0.1:9998/jsonrpc";
const POOL_SIZE_DEFAULT: u32 = 5;

#[derive(Deserialize, Clone)]
pub struct ThunderPlatformParameters {
    #[serde(default = "gateway_default")]
    gateway: String,
    #[serde(default = "pool_size_default")]
    pool_size: u32,
}

fn gateway_default() -> String {
    String::from(GATEWAY_DEFAULT)
}

fn pool_size_default() -> u32 {
    POOL_SIZE_DEFAULT
}

impl ThunderGetConfigStep {
    pub fn get_name() -> String {
        "ThunderGetConfigStep".into()
    }

    pub async fn setup(
        mut state: ExtnClient,
        expected_plugins: ThunderPluginBootParam,
    ) -> Result<ThunderBootstrapStateWithConfig, RippleError> {
        debug!("Requesting Platform parameters");
        let extn_message_response: Result<ExtnMessage, RippleError> =
            state.request(Config::PlatformParameters).await;
        if let Ok(message) = extn_message_response {
            if let Some(ExtnResponse::Value(v)) = message.payload.extract() {
                let mut pool_size = POOL_SIZE_DEFAULT;
                let tp_res: Result<ThunderPlatformParameters, Error> = serde_json::from_value(v);
                let mut gateway_url = url::Url::parse(GATEWAY_DEFAULT).unwrap();
                if let Ok(thunder_parameters) = tp_res {
                    pool_size = thunder_parameters.pool_size;
                    if let Ok(gurl) = url::Url::parse(&thunder_parameters.gateway) {
                        debug!("Got url from device manifest");
                        gateway_url = gurl
                    } else {
                        warn!(
                            "Could not parse thunder gateway '{}', using default {}",
                            thunder_parameters.gateway, GATEWAY_DEFAULT
                        );
                    }
                } else {
                    warn!(
                        "Could not read thunder platform parameters, using default {}",
                        GATEWAY_DEFAULT
                    );
                }
                if let Ok(host_override) = std::env::var("DEVICE_HOST") {
                    gateway_url.set_host(Some(&host_override)).ok();
                }
                return Ok(ThunderBootstrapStateWithConfig {
                    extn_client: state,
                    url: gateway_url,
                    pool_size,
                    plugin_param: expected_plugins,
                    thunder_connection_state: Arc::new(ThunderConnectionState::new()),
                });
            }
        }

        Err(RippleError::BootstrapError)
    }
}
