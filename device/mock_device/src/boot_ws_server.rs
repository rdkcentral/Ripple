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

// use crate::{
//     bootstrap::setup_thunder_processors::SetupThunderProcessor,
//     client::plugin_manager::ThunderPluginBootParam, thunder_state::ThunderBootstrapStateWithClient,
// };
// use ripple_sdk::{extn::client::extn_client::ExtnClient, log::info};

// use super::{get_config_step::ThunderGetConfigStep, setup_thunder_pool_step::ThunderPoolStep};

use std::collections::HashMap;

use ripple_sdk::{
    api::config::Config,
    extn::{client::extn_client::ExtnClient, extn_client_message::ExtnResponse},
    log::debug,
    utils::error::RippleError,
};
use url::{Host, Url};

use crate::mock_ws_server::{MockWebsocketServer, WsServerParameters};

pub async fn boot_ws_server(mut client: ExtnClient) -> Result<MockWebsocketServer, RippleError> {
    debug!("Booting WS Server for mock device");
    let gateway = platform_gateway(&mut client).await?;

    if gateway.scheme() != "ws" {
        // TODO:: add proper error
        return Err(RippleError::ParseError);
    }
    // let host = gateway.host();

    if !is_valid_host(gateway.host()) {
        // TODO: check host
        return Err(RippleError::ParseError);
    }

    let server_config = WsServerParameters::new();
    server_config
        .port(gateway.port().unwrap_or(0))
        .path(gateway.path());
    let ws_server = MockWebsocketServer::new(HashMap::new(), server_config)
        .await
        .map_err(|_e| RippleError::BootstrapError)?;

    Ok(ws_server)
}

async fn platform_gateway(client: &mut ExtnClient) -> Result<Url, RippleError> {
    if let Ok(response) = client.request(Config::PlatformParameters).await {
        if let Some(ExtnResponse::Value(value)) = response.payload.extract() {
            let gateway: Url = value
                .as_object()
                .and_then(|obj| obj.get("gateway"))
                .and_then(|val| val.as_str())
                .and_then(|s| s.parse().ok())
                .ok_or(RippleError::ParseError)?;
            debug!("{}", gateway);
            return Ok(gateway);
        }
    }

    Err(RippleError::ParseError)
}

fn is_valid_host(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Ipv4(ipv4)) => ipv4.is_loopback() || ipv4.is_unspecified(),
        _ => return false,
    }
}
