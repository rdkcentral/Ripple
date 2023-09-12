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

use std::{
    collections::HashMap,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use ripple_sdk::{
    api::config::Config,
    extn::{
        client::extn_client::ExtnClient,
        extn_client_message::{ExtnRequest, ExtnResponse},
    },
    log::{debug, error},
    tokio::{self},
    utils::error::RippleError,
};
use serde_hashkey::{to_key, Key};
use serde_json::Value;
use url::{Host, Url};

use crate::mock_ws_server::{MockWebsocketServer, WsServerParameters};

#[derive(Clone, Debug)]
pub enum BootWsServerError {
    BadUrlScheme,
    BadHostname,
    GetPlatformGatewayFailed,
    ServerStartFailed,
}

pub async fn boot_ws_server(
    mut client: ExtnClient,
) -> Result<Arc<MockWebsocketServer>, BootWsServerError> {
    debug!("Booting WS Server for mock device");
    let gateway = platform_gateway(&mut client).await?;

    if gateway.scheme() != "ws" {
        return Err(BootWsServerError::BadUrlScheme);
    }

    if !is_valid_host(gateway.host()) {
        return Err(BootWsServerError::BadHostname);
    }

    let mock_data = load_mock_data(client.clone())
        .await
        .map_err(|e| {
            error!("{:?}", e);
            e
        })
        .unwrap_or_default();
    debug!("mock_data={:?}", mock_data);

    let mut server_config = WsServerParameters::new();
    server_config
        .port(gateway.port().unwrap_or(0))
        .path(gateway.path());
    let ws_server = MockWebsocketServer::new(mock_data, server_config)
        .await
        .map_err(|_e| BootWsServerError::ServerStartFailed)?;

    let ws_server = Arc::new(ws_server);
    let server = ws_server.clone();

    tokio::spawn(async move {
        server.start_server().await;
    });

    Ok(ws_server)
}

async fn platform_gateway(client: &mut ExtnClient) -> Result<Url, BootWsServerError> {
    if let Ok(response) = client.request(Config::PlatformParameters).await {
        if let Some(ExtnResponse::Value(value)) = response.payload.extract() {
            let gateway: Url = value
                .as_object()
                .and_then(|obj| obj.get("gateway"))
                .and_then(|val| val.as_str())
                .and_then(|s| s.parse().ok())
                .ok_or(BootWsServerError::GetPlatformGatewayFailed)?;
            debug!("{}", gateway);
            return Ok(gateway);
        }
    }

    Err(BootWsServerError::GetPlatformGatewayFailed)
}

fn is_valid_host(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Ipv4(ipv4)) => ipv4.is_loopback() || ipv4.is_unspecified(),
        _ => false,
    }
}

#[derive(Clone, Debug)]
pub enum LoadMockDataError {
    PathDoesNotExist(PathBuf),
    FileOpenFailed(PathBuf),
    GetSavedDirFailed,
    MockDataNotValidJson,
    MockDataNotArray,
    EntryNotObject,
    EntryMissingRequestField,
    EntryMissingResponseField,
}

async fn load_mock_data(
    mut client: ExtnClient,
) -> Result<HashMap<Key, Vec<Value>>, LoadMockDataError> {
    debug!("requesting saved dir");
    let saved_dir = client
        .request(Config::SavedDir)
        .await
        .and_then(|response| -> Result<PathBuf, RippleError> {
            if let Some(ExtnResponse::String(value)) = response.payload.extract() {
                if let Ok(buf) = value.parse::<PathBuf>() {
                    return Ok(buf);
                }
            }

            Err(RippleError::ParseError)
        })
        .map_err(|e| {
            error!("Config::SaveDir request error {:?}", e);
            LoadMockDataError::GetSavedDirFailed
        })?;

    debug!("received saved_dir {saved_dir:?}");
    if !saved_dir.is_dir() {
        return Err(LoadMockDataError::PathDoesNotExist(saved_dir));
    }

    let path = saved_dir.join("mock-device.json");
    debug!("path={:?}", path);
    if !path.is_file() {
        return Err(LoadMockDataError::PathDoesNotExist(path));
    }

    let file = File::open(path.clone()).map_err(|e| {
        error!("Failed to open mock data file {e:?}");
        LoadMockDataError::FileOpenFailed(path)
    })?;
    let reader = BufReader::new(file);
    let json: serde_json::Value =
        serde_json::from_reader(reader).map_err(|_| LoadMockDataError::MockDataNotValidJson)?;

    if let Some(list) = json.as_array() {
        let map = list
            .iter()
            .map(|req_resp| {
                // TODO: validate as JSONRPC
                let obj = req_resp
                    .as_object()
                    .ok_or(LoadMockDataError::EntryNotObject)?;
                let req = obj
                    .get("request")
                    .and_then(|req| if req.is_object() { Some(req) } else { None })
                    // .and_then(|req_obj| serde_json::to_string(req_obj).ok())
                    .ok_or(LoadMockDataError::EntryMissingRequestField)?;
                let res = obj
                    .get("response")
                    .and_then(|res| if res.is_object() { Some(res) } else { None })
                    // .and_then(|req_obj| serde_json::to_string(req_obj).ok())
                    .ok_or(LoadMockDataError::EntryMissingResponseField)?;

                Ok((to_key(req).unwrap(), vec![res.to_owned()]))
                // TODO: add support for multiple responses
            })
            .collect::<Result<Vec<(Key, Vec<Value>)>, LoadMockDataError>>()?
            .into_iter()
            .collect();

        Ok(map)
    } else {
        Err(LoadMockDataError::MockDataNotArray)
    }
}
