use std::{fmt::Display, path::PathBuf};

use crate::mock_data::MockDataError;

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
    LoadMockDataFailed(LoadMockDataFailedReason),
}

impl Display for MockDeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BootFailed(reason) => f.write_fmt(format_args!(
                "Failed to start websocket server. Reason: {reason}"
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
    MockDataError(MockDataError),
}
impl Display for LoadMockDataFailedReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PathDoesNotExist(path) => f.write_fmt(format_args!(
                "Path does not exist. Path: {}",
                path.display()
            )),
            Self::FileOpenFailed(path) => f.write_fmt(format_args!(
                "Failed to open file. File: {}",
                path.display()
            )),
            Self::GetSavedDirFailed => f.write_str("Failed to get SavedDir from config."),
            Self::MockDataNotValidJson => f.write_str("The mock data is not valid JSON."),
            Self::MockDataNotArray => {
                f.write_str("The mock data file root object must be an array.")
            }
            Self::MockDataError(err) => f.write_fmt(format_args!(
                "Failed to parse message in mock data. Error: {err:?}"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::errors::MockWebsocketServerError;

    #[test]
    fn test_mock_websocket_server_error_display() {
        let error = MockWebsocketServerError::CantListen;

        assert_eq!("Failed to start TcpListener", error.to_string());
    }
}
