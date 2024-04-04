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

use std::{fmt::Display, path::PathBuf};

use crate::mock_data::MockDataError;

#[derive(Debug, Clone)]
pub enum MockServerWebSocketError {
    CantListen,
}

impl std::error::Error for MockServerWebSocketError {}

impl Display for MockServerWebSocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::CantListen => "Failed to start TcpListener",
        };

        f.write_str(msg)
    }
}

#[derive(Clone, Debug)]
pub enum MockDeviceError {
    BootFailed(BootFailedError),
    LoadMockDataFailed(LoadMockDataError),
}

impl std::error::Error for MockDeviceError {}

impl Display for MockDeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::BootFailed(reason) => {
                format!("Failed to start websocket server. Reason: {reason}")
            }
            Self::LoadMockDataFailed(reason) => {
                format!("Failed to load mock data from file. Reason: {reason}")
            }
        };

        f.write_str(msg.as_str())
    }
}

#[derive(Clone, Debug)]
pub enum BootFailedError {
    BadUrlScheme,
    BadHostname,
    GetPlatformGatewayFailed,
    ServerStartFailed(MockServerWebSocketError),
}

impl Display for BootFailedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::BadUrlScheme => "The scheme in the URL is invalid. It must be `ws`.".to_owned(),
            Self::BadHostname => {
                "The hostname in the URL is invalid. It must be `0.0.0.0` or `127.0.0.1`."
                    .to_owned()
            }
            Self::GetPlatformGatewayFailed => {
                "Failed to get plaftform gateway from the Thunder extension config.".to_owned()
            }
            Self::ServerStartFailed(err) => {
                format!("Failed to start the WebSocket server. Error: {err}")
            }
        };

        f.write_str(msg.as_str())
    }
}

impl From<BootFailedError> for MockDeviceError {
    fn from(value: BootFailedError) -> Self {
        Self::BootFailed(value)
    }
}

#[derive(Clone, Debug)]
pub enum LoadMockDataError {
    PathDoesNotExist(PathBuf),
    FileOpenFailed(PathBuf),
    GetSavedDirFailed,
    MockDataNotValidJson,
    MockDataNotArray,
    MockDataError(MockDataError),
}

impl Display for LoadMockDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::PathDoesNotExist(path) => {
                format!("Path does not exist. Path: {}", path.display())
            }
            Self::FileOpenFailed(path) => format!("Failed to open file. File: {}", path.display()),
            Self::GetSavedDirFailed => "Failed to get SavedDir from config.".to_owned(),
            Self::MockDataNotValidJson => "The mock data is not valid JSON.".to_owned(),
            Self::MockDataNotArray => "The mock data file root object must be an array.".to_owned(),
            Self::MockDataError(err) => {
                format!("Failed to parse message in mock data. Error: {err:?}")
            }
        };

        f.write_str(msg.as_str())
    }
}

impl From<LoadMockDataError> for MockDeviceError {
    fn from(value: LoadMockDataError) -> Self {
        Self::LoadMockDataFailed(value)
    }
}
