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
    api::config::Config,
    extn::{client::extn_client::ExtnClient, extn_client_message::ExtnResponse},
    log::{debug, error},
    tokio,
    utils::error::RippleError,
};
use serde_json::Value;
use std::collections::HashMap;
use std::net::TcpListener;
use std::{fs::File, io::BufReader, path::PathBuf, sync::Arc};
use url::{Host, Url};

use crate::mock::errors::MockServerWebSocketError;
use crate::mock::mock_data::ParamResponse;
use crate::mock::{
    errors::{BootFailedError, LoadMockDataError, MockDeviceError},
    mock_config::MockConfig,
    mock_data::MockData,
    mock_web_socket_server::{MockWebSocketServer, WsServerParameters},
};
fn get_available_port() -> Option<u16> {
    (3000..10000).find(|port| port_is_available(*port))
}

fn port_is_available(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => true,
        Err(_) => false,
    }
}
pub fn load_mock_data_v2_from_file(path: &PathBuf) -> Result<MockData, MockDeviceError> {
    let file = File::open(path).map_err(|e| {
        error!("Failed to open mock data file {e:?}");
        LoadMockDataError::FileOpenFailed(path.to_owned())
    })?;
    let reader = BufReader::new(file);

    if let Ok(v) = serde_json::from_reader(reader) {
        return Ok(v);
    }
    Err(MockDeviceError::LoadMockDataFailed(
        LoadMockDataError::MockDataNotValidJson,
    ))
}
pub async fn boot_for_unit_test(
    mock_data_v2: MockData,
) -> Result<(Url, Arc<MockWebSocketServer>), MockDeviceError> {
    let port = get_available_port().ok_or(MockDeviceError::NoAvailablePort)?;
    let hostname = "localhost";
    let uri = "jsonrpc";
    let gateway_url = Url::parse(&format!("ws://{}:{}/{}", hostname, port, uri)).map_err(|e| {
        MockDeviceError::BadUrlScheme(
            format!("{} for ws://{}:{}/{}", e, hostname, port, uri).to_string(),
        )
    })?;
    Ok((
        gateway_url.clone(),
        boot_ws_server(
            MockConfig::default(),
            gateway_url,
            mock_data_v2,
            WsServerParameters::new().with_port(port),
        )
        .await?,
    ))
}

pub async fn boot_ws_server(
    server_config: MockConfig,
    gateway_url: Url,
    mock_data_v2: MockData,
    ws_server_params: &mut WsServerParameters,
) -> Result<Arc<MockWebSocketServer>, MockDeviceError> {
    let params = ws_server_params
        .with_port(gateway_url.port().unwrap_or(0))
        .with_path(gateway_url.path())
        .to_owned();
    let ws_server = MockWebSocketServer::new(mock_data_v2, params, server_config)
        .await
        .map_err(BootFailedError::ServerStartFailed)?;

    let ws_server = Arc::new(ws_server);
    let server = ws_server.clone();

    tokio::spawn(async move {
        server.start_server().await;
    });

    Ok(ws_server)
}
pub async fn start_ws_server(
    mut client: ExtnClient,
) -> Result<Arc<MockWebSocketServer>, MockDeviceError> {
    debug!("Booting WS Server for mock device");
    let gateway_url = platform_gateway_url(&mut client).await?;

    if gateway_url.scheme() != "ws" {
        return Err(BootFailedError::BadUrlScheme)?;
    }

    if !is_valid_host(gateway_url.host()) {
        return Err(BootFailedError::BadHostname)?;
    }

    let mock_config = load_config(&client);

    let mut ws_server_params = WsServerParameters::new();
    let mock_data_v2 = load_mock_data_v2(client.clone()).await?;

    Ok(boot_ws_server(
        mock_config,
        gateway_url,
        mock_data_v2,
        &mut ws_server_params,
    )
    .await?)
}

async fn platform_gateway_url(client: &mut ExtnClient) -> Result<Url, MockDeviceError> {
    debug!("sending request for config.platform_parameters");
    if let Ok(response) = client.request(Config::PlatformParameters).await {
        if let Some(ExtnResponse::Value(value)) = response.payload.extract() {
            let gateway: Url = value
                .as_object()
                .and_then(|obj| obj.get("gateway"))
                .and_then(|val| val.as_str())
                .and_then(|s| s.parse().ok())
                .ok_or(BootFailedError::GetPlatformGatewayFailed)?;
            debug!("{}", gateway);
            return Ok(gateway);
        }
    }

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
        return Err(LoadMockDataError::PathDoesNotExist(saved_dir))?;
    }

    let path = saved_dir.join(path);

    Ok(path)
}

pub fn load_config(client: &ExtnClient) -> MockConfig {
    match client
        .get_config("key")
        .unwrap_or("true".into())
        .parse::<bool>()
    {
        Ok(v) => MockConfig::default()
            .with_activate_all_plugins(v)
            .to_owned(),
        Err(e) => {
            error!("Failed to parse activate_all_plugins config value: {e:?}");
            MockConfig::default()
        }
    }
}
pub fn create_mock_config(activate_all_plugins: bool) -> MockConfig {
    MockConfig::default()
        .with_activate_all_plugins(activate_all_plugins)
        .to_owned()
}

pub async fn load_mock_data_v2(client: ExtnClient) -> Result<MockData, MockDeviceError> {
    let path = find_mock_device_data_file(client).await?;
    load_mock_data_v2_from_file(&path)
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
    #[test]
    fn test_boot_for_unit_test() {
        let mock_data = MockData::default();
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(boot_for_unit_test(mock_data.clone()));
        let r = result.unwrap();
        println!("url={}", r.0);

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(boot_for_unit_test(mock_data));
        let r = result.unwrap();
        println!("url={}", r.0);
    }
}
