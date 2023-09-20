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

use std::{collections::HashMap, fs::File, io::BufReader, path::PathBuf, sync::Arc};

use ripple_sdk::{
    api::config::Config,
    extn::{client::extn_client::ExtnClient, extn_client_message::ExtnResponse},
    log::{debug, error},
    tokio::{self, sync::Mutex},
    utils::error::RippleError,
};
use serde_hashkey::{to_key, Key};
use serde_json::Value;
use url::{Host, Url};

use crate::{
    errors::{BootFailedReason, LoadMockDataFailedReason, MockDeviceError},
    mock_ws_server::{MockWebsocketServer, WsServerParameters},
};

pub type MockData = HashMap<Key, Vec<Value>>;

pub async fn boot_ws_server(
    mut client: ExtnClient,
    mock_data: Mutex<MockData>,
) -> Result<Arc<MockWebsocketServer>, MockDeviceError> {
    debug!("Booting WS Server for mock device");
    let gateway = platform_gateway_url(&mut client).await?;

    if gateway.scheme() != "ws" {
        return Err(MockDeviceError::BootFailed(BootFailedReason::BadUrlScheme));
    }

    if !is_valid_host(gateway.host()) {
        return Err(MockDeviceError::BootFailed(BootFailedReason::BadHostname));
    }

    let mut server_config = WsServerParameters::new();
    server_config
        .port(gateway.port().unwrap_or(0))
        .path(gateway.path());
    let ws_server = MockWebsocketServer::new(mock_data, server_config)
        .await
        .map_err(|e| MockDeviceError::BootFailed(BootFailedReason::ServerStartFailed(e)))?;

    let ws_server = Arc::new(ws_server);
    let server = ws_server.clone();

    tokio::spawn(async move {
        server.start_server().await;
    });

    Ok(ws_server)
}

async fn platform_gateway_url(client: &mut ExtnClient) -> Result<Url, MockDeviceError> {
    if let Ok(response) = client.request(Config::PlatformParameters).await {
        if let Some(ExtnResponse::Value(value)) = response.payload.extract() {
            let gateway: Url = value
                .as_object()
                .and_then(|obj| obj.get("gateway"))
                .and_then(|val| val.as_str())
                .and_then(|s| s.parse().ok())
                .ok_or(MockDeviceError::BootFailed(
                    BootFailedReason::GetPlatformGatewayFailed,
                ))?;
            debug!("{}", gateway);
            return Ok(gateway);
        }
    }

    Err(MockDeviceError::BootFailed(
        BootFailedReason::GetPlatformGatewayFailed,
    ))
}

fn is_valid_host(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Ipv4(ipv4)) => ipv4.is_loopback() || ipv4.is_unspecified(),
        _ => false,
    }
}

pub async fn load_mock_data(mut client: ExtnClient) -> Result<MockData, MockDeviceError> {
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
            MockDeviceError::LoadMockDataFailed(LoadMockDataFailedReason::GetSavedDirFailed)
        })?;

    debug!("received saved_dir {saved_dir:?}");
    if !saved_dir.is_dir() {
        return Err(MockDeviceError::LoadMockDataFailed(
            LoadMockDataFailedReason::PathDoesNotExist(saved_dir),
        ));
    }

    let path = saved_dir.join("mock-device.json"); // TODO: allow this to be overridden in config
    debug!("path={:?}", path);
    if !path.is_file() {
        return Err(MockDeviceError::LoadMockDataFailed(
            LoadMockDataFailedReason::PathDoesNotExist(path),
        ));
    }

    let file = File::open(path.clone()).map_err(|e| {
        error!("Failed to open mock data file {e:?}");
        MockDeviceError::LoadMockDataFailed(LoadMockDataFailedReason::FileOpenFailed(path))
    })?;
    let reader = BufReader::new(file);
    let json: serde_json::Value = serde_json::from_reader(reader).map_err(|_| {
        MockDeviceError::LoadMockDataFailed(LoadMockDataFailedReason::MockDataNotValidJson)
    })?;

    if let Some(list) = json.as_array() {
        let mock_data = list
            .iter()
            .map(|req_resp| {
                let obj = req_resp
                    .as_object()
                    .ok_or(MockDeviceError::LoadMockDataFailed(
                        LoadMockDataFailedReason::EntryNotObject,
                    ))?;
                let req = obj
                    .get("request")
                    .and_then(|req| if req.is_object() { Some(req) } else { None })
                    .ok_or(MockDeviceError::LoadMockDataFailed(
                        LoadMockDataFailedReason::EntryMissingRequestField,
                    ))?;
                let res = obj
                    .get("response")
                    .and_then(|res| if res.is_object() { Some(res) } else { None })
                    .ok_or(MockDeviceError::LoadMockDataFailed(
                        LoadMockDataFailedReason::EntryMissingResponseField,
                    ))?;

                Ok((json_key(req)?, vec![res.to_owned()]))
                // TODO: add support for multiple responses
            })
            .collect::<Result<Vec<(Key, Vec<Value>)>, MockDeviceError>>()?
            .into_iter()
            .collect::<HashMap<Key, Vec<Value>>>();

        Ok(mock_data)
    } else {
        Err(MockDeviceError::LoadMockDataFailed(
            LoadMockDataFailedReason::MockDataNotArray,
        ))
    }
}

pub fn json_key(value: &Value) -> Result<Key, MockDeviceError> {
    let key = to_key(value);
    if let Ok(key) = key {
        return Ok(key);
    }

    error!("Failed to create key from data {value:?}");
    Err(MockDeviceError::BadMockDataKey(value.clone()))
}
