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

use std::{fs::File, io::BufReader, path::PathBuf, sync::Arc};

use ripple_sdk::{
    api::config::Config,
    extn::{client::extn_client::ExtnClient, extn_client_message::ExtnResponse},
    log::{debug, error},
    tokio,
    utils::error::RippleError,
};
use serde_json::Value;
use url::{Host, Url};

use crate::{
    errors::{BootFailedError, LoadMockDataError, MockDeviceError},
    mock_config::MockConfig,
    mock_data::MockData,
    mock_web_socket_server::{MockWebSocketServer, WsServerParameters},
};

pub async fn boot_ws_server(
    mut client: ExtnClient,
) -> Result<Arc<MockWebSocketServer>, MockDeviceError> {
    debug!("@@@NNA Booting WS Server for mock device");
    debug!(
        "@@@NNA Booting WS Server for mock device client:{:?}",
        client
    );
    let gateway = platform_gateway_url(&mut client).await?;
    debug!("@@@NNA platform_gateway_url returned: {:?}", gateway);

    if gateway.scheme() != "ws" {
        debug!("@@@NNA Bad URL scheme: {}", gateway.scheme());
        return Err(BootFailedError::BadUrlScheme)?;
    }

    if !is_valid_host(gateway.host()) {
        debug!("@@@NNA Bad hostname: {:?}", gateway.host());
        return Err(BootFailedError::BadHostname)?;
    }

    let config = load_config(&client);
    debug!("@@@NNA Loaded config: {:?}", config);

    let mut server_config = WsServerParameters::new();
    let mock_data_v2 = load_mock_data_v2(client.clone()).await?;
    debug!("@@@NNA Loaded mock data v2");

    server_config
        .port(gateway.port().unwrap_or(0))
        .path(gateway.path());
    debug!(
        "@@@NNA Server config: port={}, path={}",
        gateway.port().unwrap_or(0),
        gateway.path()
    );

    let ws_server = MockWebSocketServer::new(mock_data_v2, server_config, config)
        .await
        .map_err(BootFailedError::ServerStartFailed)?;
    debug!("@@@NNA MockWebSocketServer created");

    let ws_server = Arc::new(ws_server);
    let server = ws_server.clone();

    tokio::spawn(async move {
        debug!("@@@NNA Starting WebSocket server");
        server.start_server().await;
    });

    debug!("@@@NNA Returning ws_server Arc");
    Ok(ws_server)
}

async fn platform_gateway_url(client: &mut ExtnClient) -> Result<Url, MockDeviceError> {
    debug!(
        "@@@NNA platform_gateway_url called with client: {:?}",
        client
    );
    debug!("@@@NNA sending request for config.platform_parameters");
    if let Ok(response) = client.request(Config::PlatformParameters).await {
        debug!("@@@NNA received response: {:?}", response);
        if let Some(ExtnResponse::Value(value)) = response.payload.extract() {
            debug!("@@@NNA extracted value: {:?}", value);
            let gateway: Url = value
                .as_object()
                .and_then(|obj| obj.get("gateway"))
                .and_then(|val| val.as_str())
                .and_then(|s| s.parse().ok())
                .ok_or(BootFailedError::GetPlatformGatewayFailed)?;
            debug!("@@@NNA parsed gateway URL: {}", gateway);
            return Ok(gateway);
        }
    }

    error!("@@@NNA Failed to get platform gateway URL");
    Err(BootFailedError::GetPlatformGatewayFailed)?
}

fn is_valid_host(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Ipv4(ipv4)) => ipv4.is_loopback() || ipv4.is_unspecified(),
        _ => false,
    }
}

async fn find_mock_device_data_file(mut client: ExtnClient) -> Result<PathBuf, MockDeviceError> {
    let file = client
        .get_config("mock_data_file")
        .unwrap_or("mock-device.json".to_owned());
    let path = PathBuf::from(file);

    debug!(
        "mock data path={} absolute={}",
        path.display(),
        path.is_absolute()
    );

    if path.is_absolute() {
        return Ok(path);
    }

    let saved_dir = client
        .request(Config::SavedDir)
        .await
        .and_then(|response| -> Result<PathBuf, RippleError> {
            if let Some(ExtnResponse::String(value)) = response.payload.extract() {
                let Ok(buf) = value.parse::<PathBuf>();
                return Ok(buf);
            }

            Err(RippleError::ParseError)
        })
        .map_err(|e| {
            error!("Config::SaveDir request error {:?}", e);
            LoadMockDataError::GetSavedDirFailed
        })?;

    debug!("received saved_dir {saved_dir:?}");
    if !saved_dir.is_dir() {
        return Err(LoadMockDataError::PathDoesNotExist(saved_dir))?;
    }

    let path = saved_dir.join(path);

    Ok(path)
}

pub fn load_config(client: &ExtnClient) -> MockConfig {
    let mut config = MockConfig::default();

    if let Some(c) = client.get_config("activate_all_plugins") {
        config.activate_all_plugins = c.parse::<bool>().unwrap_or(false);
    }
    config
}

pub async fn load_mock_data_v2(client: ExtnClient) -> Result<MockData, MockDeviceError> {
    let path = find_mock_device_data_file(client).await?;
    debug!("path={:?}", path);
    if !path.is_file() {
        return Err(LoadMockDataError::PathDoesNotExist(path))?;
    }

    let file = File::open(path.clone()).map_err(|e| {
        error!("Failed to open mock data file {e:?}");
        LoadMockDataError::FileOpenFailed(path)
    })?;
    let reader = BufReader::new(file);

    if let Ok(v) = serde_json::from_reader(reader) {
        return Ok(v);
    }
    Err(MockDeviceError::LoadMockDataFailed(
        LoadMockDataError::MockDataNotValidJson,
    ))
}

pub fn is_value_jsonrpc(value: &Value) -> bool {
    value.as_object().map_or(false, |req| {
        req.contains_key("jsonrpc") && req.contains_key("id") && req.contains_key("method")
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_is_value_jsonrpc_true() {
        assert!(is_value_jsonrpc(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "someAction", "params": {}})
        ));
    }

    #[test]
    fn test_is_value_jsonrpc_false() {
        assert!(!is_value_jsonrpc(&json!({"key": "value"})));
    }
}
