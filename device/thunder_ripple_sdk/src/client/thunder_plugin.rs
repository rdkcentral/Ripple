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

use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use ThunderPluginConfig as Cfg;

#[derive(Debug, EnumIter)]
pub enum ThunderPlugin {
    Controller,
    DeviceInfo,
    DisplaySettings,
    LocationSync,
    Network,
    RDKShell,
    RemoteControl,
    PersistentStorage,
    System,
    Wifi,
    TextToSpeech,
    Hdcp,
}
const CONTROLLER_CFG: Cfg = Cfg::new("Controller", false, true);
const DEVICE_INFO_CFG: Cfg = Cfg::new("DeviceInfo", true, false);
const DISPLAY_SETTINGS_CFG: Cfg = Cfg::new("org.rdk.DisplaySettings", true, false);
const HDCP_CFG: Cfg = Cfg::new("org.rdk.HdcpProfile", true, false);
const NETWORK_CFG: Cfg = Cfg::new("org.rdk.Network", true, false);
const PERSISTENT_STORAGE_CFG: Cfg = Cfg::new("org.rdk.PersistentStore", false, false);
const RDKSHELL_CFG: Cfg = Cfg::new("org.rdk.RDKShell", false, false);
const REMOTE_CONTROL_CFG: Cfg = Cfg::new("org.rdk.RemoteControl", false, false);
const SYSTEM_CFG: Cfg = Cfg::new("org.rdk.System", true, false);
const WIFI_CFG: Cfg = Cfg::new("org.rdk.Wifi", false, false);
const LOCATION_SYNC: Cfg = Cfg::new("LocationSync", false, false);
const TTS_CFG: Cfg = Cfg::new("org.rdk.TextToSpeech", false, true);

impl ThunderPlugin {
    pub fn cfg(&self) -> Cfg {
        use ThunderPlugin::*;
        match self {
            Controller => CONTROLLER_CFG,
            DeviceInfo => DEVICE_INFO_CFG,
            DisplaySettings => DISPLAY_SETTINGS_CFG,
            Hdcp => HDCP_CFG,
            Network => NETWORK_CFG,
            PersistentStorage => PERSISTENT_STORAGE_CFG,
            RDKShell => RDKSHELL_CFG,
            RemoteControl => REMOTE_CONTROL_CFG,
            System => SYSTEM_CFG,
            Wifi => WIFI_CFG,
            LocationSync => LOCATION_SYNC,
            TextToSpeech => TTS_CFG,
        }
    }
    pub fn callsign(&self) -> &str {
        self.cfg().callsign
    }
    pub fn callsign_and_version(&self) -> String {
        format!("{}.1", self.cfg().callsign)
    }
    pub fn callsign_string(&self) -> String {
        String::from(self.callsign())
    }
    pub fn activate_at_boot(&self) -> bool {
        self.cfg().activate_at_boot
    }
    pub fn expect_activated(&self) -> bool {
        self.cfg().expect_activated
    }
    pub fn activate_on_boot_plugins() -> Vec<ThunderPlugin> {
        ThunderPlugin::iter()
            .filter(|p| p.activate_at_boot())
            .collect::<Vec<_>>()
    }
    pub fn expect_activated_plugins() -> Vec<ThunderPlugin> {
        ThunderPlugin::iter()
            .filter(|p| p.expect_activated())
            .collect::<Vec<_>>()
    }
    pub fn method(&self, method_name: &str) -> String {
        format!("{}.1.{}", self.callsign(), method_name)
    }
    pub fn method_version(&self, method_name: &str, version: u32) -> String {
        format!("{}.{}.{}", self.callsign(), version, method_name)
    }
}

pub struct ThunderPluginConfig {
    callsign: &'static str,
    activate_at_boot: bool,
    expect_activated: bool,
}

impl ThunderPluginConfig {
    pub const fn new(
        callsign: &'static str,
        activate_at_boot: bool,
        expect_activated: bool,
    ) -> ThunderPluginConfig {
        ThunderPluginConfig {
            callsign,
            activate_at_boot,
            expect_activated,
        }
    }
}
