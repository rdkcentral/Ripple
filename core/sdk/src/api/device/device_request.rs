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
use serde::{Deserialize, Serialize};

use super::{
    device_browser::BrowserRequest, device_info_request::DeviceInfoRequest,
    device_window_manager::WindowManagerRequest, device_wifi::WifiRequest,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceRequest {
    DeviceInfo(DeviceInfoRequest),
    Browser(BrowserRequest),
    WindowManager(WindowManagerRequest),
<<<<<<< HEAD
    Extn(BaseDeviceRequest),
    Wifi(WifiRequest)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseDeviceRequest {
    pub params: Option<Value>,
    pub method: String,
    pub module: String,
}

impl ExtnPayloadProvider for BaseDeviceRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Extn(self.clone())))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(request) => match request {
                ExtnRequest::Device(r) => match r {
                    DeviceRequest::Extn(d) => return Some(d),
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn get_capability(&self) -> ExtnCapability {
        ExtnCapability::new_channel(ExtnClass::Device, self.method.clone())
    }

    fn cap() -> ExtnCapability {
        ExtnCapability::new_channel(ExtnClass::Device, "info".into())
    }
=======
>>>>>>> main
}
