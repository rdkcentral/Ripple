// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
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
use crate::utils::error::RippleError;

#[derive(Clone, Debug)]
pub enum RippleContract {
    Internal,
    Main(MainContract),
    Session,
    Device(DeviceContract),
    Distributor,
    Governance,
    Discovery,
    Launcher,
    JsonRpsee,
}

impl RippleContract {
    pub fn get_short(&self) -> Option<String> {
        match self {
            Self::Device(_) => Some("device".into()),
            Self::Main(_) => Some("main".into()),
            _ => None,
        }
    }

    pub fn is_main(&self) -> bool {
        match self {
            Self::Main(_) | Self::Internal => true,
            _ => false,
        }
    }
}

impl TryFrom<String> for RippleContract {
    type Error = RippleError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let c_a = value.split(":");
        if c_a.count() == 2 {
            let c_a: Vec<&str> = value.split(":").collect();
            return match c_a.get(0).unwrap().to_lowercase().as_str() {
                "device" => {
                    if let Ok(v) = DeviceContract::try_from(c_a.get(1).unwrap().to_lowercase()) {
                        Ok(Self::Device(v))
                    } else {
                        Err(RippleError::ParseError)
                    }
                }
                "main" => {
                    if let Ok(v) = MainContract::try_from(c_a.get(1).unwrap().to_lowercase()) {
                        Ok(Self::Main(v))
                    } else {
                        Err(RippleError::ParseError)
                    }
                }
                _ => Err(RippleError::ParseError),
            };
        } else {
            match value.as_str() {
                "launcher" => Ok(Self::Launcher),
                _ => Err(RippleError::ParseError),
            }
        }
    }
}

impl Into<String> for RippleContract {
    fn into(self) -> String {
        match self {
            Self::Device(cap) => format!("device:{:?}", cap).to_lowercase(),
            Self::Main(cap) => format!("main:{:?}", cap).to_lowercase(),
            _ => format!("{:?}", self).to_lowercase(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum DeviceContract {
    Info,
    WindowManager,
    Browser,
}

impl TryFrom<String> for DeviceContract {
    type Error = RippleError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "info" => Ok(Self::Info),
            "windowmanager" => Ok(Self::WindowManager),
            "browser" => Ok(Self::Browser),
            _ => Err(RippleError::ParseError),
        }
    }
}

#[derive(Clone, Debug)]
pub enum MainContract {
    Config,
    LifecycleManagement,
    Rpc,
    ExtnStatus,
}

impl TryFrom<String> for MainContract {
    type Error = RippleError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "config" => Ok(Self::Config),
            "lifecyclemanagement" => Ok(Self::LifecycleManagement),
            "rpc" => Ok(Self::Rpc),
            "extnstatus" => Ok(Self::ExtnStatus),
            _ => Err(RippleError::ParseError),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::framework::ripple_contract::{DeviceContract, RippleContract};

    #[test]
    fn test_into() {
        let value: String =
            RippleContract::Device(crate::framework::ripple_contract::DeviceContract::Info).into();
        println!("{}", value);
        assert!(value.eq("device:info"));
        let result = RippleContract::try_from(value);
        assert!(result.is_ok());
        assert!(if let Ok(RippleContract::Device(cap)) = result {
            if let DeviceContract::Info = cap {
                true
            } else {
                false
            }
        } else {
            false
        });
    }
}
