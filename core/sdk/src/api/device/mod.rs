use serde::Deserialize;

pub mod device_browser;
pub mod device_info_request;
pub mod device_operator;
pub mod device_request;
pub mod device_window_manager;

/// Contains a list of available Platformtypes supported by Ripple.
#[derive(Debug, Deserialize, Clone)]
pub enum DevicePlatformType {
    /// Device platform which uses JsonRpc based websocket to perform device operations. Mored details can be found in <https://github.com/rdkcentral/Thunder>.
    Thunder,
}

impl ToString for DevicePlatformType {
    fn to_string(&self) -> String {
        match self {
            DevicePlatformType::Thunder => "thunder".into(),
        }
    }
}

pub mod thunder {
    pub mod thunder_operator;
}
