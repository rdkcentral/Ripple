use serde::Deserialize;

pub mod browser_props;

/// Contains a list of available Platformtypes supported by Ripple.
#[derive(Debug, Deserialize, Clone)]
pub enum DevicePlatformType {
    /// Device platform which uses JsonRpc based websocket to perform device operations. Mored details can be found [here][https://github.com/rdkcentral/Thunder].
    Thunder,
}

impl ToString for DevicePlatformType {
    fn to_string(&self) -> String {
        match self {
            DevicePlatformType::Thunder => "thunder".into(),
        }
    }
}
