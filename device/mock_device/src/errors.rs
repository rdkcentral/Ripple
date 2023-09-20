use std::{fmt::Display, path::PathBuf};

use serde_json::Value;

#[derive(Debug, Clone)]
pub enum MockWebsocketServerError {
    CantListen,
}

impl Display for MockWebsocketServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CantListen => f.write_str("Failed to start TcpListener"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum MockDeviceError {
    BootFailed(BootFailedReason),
    BadMockDataKey(Value),
    LoadMockDataFailed(LoadMockDataFailedReason),
}

impl Display for MockDeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BootFailed(reason) => f.write_fmt(format_args!(
                "Failed to start websocket server. Reason: {reason}"
            )),
            Self::BadMockDataKey(data) => f.write_fmt(format_args!(
                "Failed to create key for mock data. Data: {data}"
            )),
            Self::LoadMockDataFailed(reason) => f.write_fmt(format_args!(
                "Failed to load mock data from file. Reason: {reason}"
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub enum BootFailedReason {
    BadUrlScheme,
    BadHostname,
    GetPlatformGatewayFailed,
    ServerStartFailed(MockWebsocketServerError),
}
impl Display for BootFailedReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadUrlScheme => f.write_str("The scheme in the URL is invalid. It must be `ws`."),
            Self::BadHostname => f.write_str(
                "The hostname in the URL is invalid. It must be `0.0.0.0` or `127.0.0.1`.",
            ),
            Self::GetPlatformGatewayFailed => {
                f.write_str("Failed to get plaftform gateway from the Thunder extension config.")
            }
            Self::ServerStartFailed(err) => f.write_fmt(format_args!(
                "Failed to start the WebSocket server. Error: {err}"
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub enum LoadMockDataFailedReason {
    PathDoesNotExist(PathBuf),
    FileOpenFailed(PathBuf),
    GetSavedDirFailed,
    MockDataNotValidJson,
    MockDataNotArray,
    EntryNotObject,
    EntryMissingRequestField,
    EntryMissingResponseField,
}
impl Display for LoadMockDataFailedReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadMockDataFailedReason::PathDoesNotExist(path) => f.write_fmt(format_args!(
                "Path does not exist. Path: {}",
                path.display()
            )),
            LoadMockDataFailedReason::FileOpenFailed(path) => f.write_fmt(format_args!(
                "Failed to open file. File: {}",
                path.display()
            )),
            LoadMockDataFailedReason::GetSavedDirFailed => {
                f.write_str("Failed to get SavedDir from config.")
            }
            LoadMockDataFailedReason::MockDataNotValidJson => {
                f.write_str("The mock data is not valid JSON.")
            }
            LoadMockDataFailedReason::MockDataNotArray => {
                f.write_str("The mock data file root object must be an array.")
            }
            LoadMockDataFailedReason::EntryNotObject => {
                f.write_str("Each entry in the mock data array must be an object.")
            }
            LoadMockDataFailedReason::EntryMissingRequestField => {
                f.write_str("Each entry must have a requet field.")
            }
            LoadMockDataFailedReason::EntryMissingResponseField => {
                f.write_str("Each entry must have a response field.")
            }
        }
    }
}
