use serde::{Deserialize, Serialize};

use super::{
    device_browser::BrowserRequest, device_info_request::DeviceInfoRequest,
    device_window_manager::WindowManagerRequest,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceRequest {
    DeviceInfo(DeviceInfoRequest),
    Browser(BrowserRequest),
    WindowManager(WindowManagerRequest),
}
