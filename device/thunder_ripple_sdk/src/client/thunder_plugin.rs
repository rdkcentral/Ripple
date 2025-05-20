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

use std::fmt;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use ThunderPluginConfig as Cfg;

#[derive(Debug, EnumIter)]
pub enum ThunderPlugin {
    Controller,
    DeviceInfo,
    DisplaySettings,
    HdcpProfile,
    LocationSync,
    Network,
    RDKShell,
    RemoteControl,
    PersistentStorage,
    System,
    Wifi,
    TextToSpeech,
    Hdcp,
    Telemetry,
    Analytics,
    UserSettings,
}
const CONTROLLER_CFG: Cfg = Cfg::new("Controller", false, true);
const DEVICE_INFO_CFG: Cfg = Cfg::new("DeviceInfo", true, false);
const DISPLAY_SETTINGS_CFG: Cfg = Cfg::new("org.rdk.DisplaySettings", true, false);
const HDCP_PROFILE_SETTINGS_CFG: Cfg = Cfg::new("org.rdk.HdcpProfile", true, false);
const HDCP_CFG: Cfg = Cfg::new("org.rdk.HdcpProfile", true, false);
const NETWORK_CFG: Cfg = Cfg::new("org.rdk.Network", false, false);
const PERSISTENT_STORAGE_CFG: Cfg = Cfg::new("org.rdk.PersistentStore", false, false);
const RDKSHELL_CFG: Cfg = Cfg::new("org.rdk.RDKShell", false, false);
const REMOTE_CONTROL_CFG: Cfg = Cfg::new("org.rdk.RemoteControl", false, false);
const SYSTEM_CFG: Cfg = Cfg::new("org.rdk.System", true, false);
const WIFI_CFG: Cfg = Cfg::new("org.rdk.Wifi", false, false);
const LOCATION_SYNC: Cfg = Cfg::new("LocationSync", false, false);
const TTS_CFG: Cfg = Cfg::new("org.rdk.TextToSpeech", false, true);
const TELEMETRY_CFG: Cfg = Cfg::new("org.rdk.Telemetry", false, false);
const ANALYTICS_CFG: Cfg = Cfg::new("org.rdk.Analytics", false, false);
const USER_SETTINGS_CFG: Cfg = Cfg::new("org.rdk.UserSettings", false, false);

impl fmt::Display for ThunderPlugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThunderPlugin::Controller => write!(f, "Controller"),
            ThunderPlugin::DeviceInfo => write!(f, "DeviceInfo"),
            ThunderPlugin::DisplaySettings => write!(f, "org.rdk.DisplaySettings"),
            ThunderPlugin::HdcpProfile => write!(f, "org.rdk.HdcpProfile"),
            ThunderPlugin::LocationSync => write!(f, "LocationSync"),
            ThunderPlugin::Network => write!(f, "org.rdk.Network"),
            ThunderPlugin::RemoteControl => write!(f, "org.rdk.RemoteControl"),
            ThunderPlugin::PersistentStorage => write!(f, "org.rdk.PersistentStore"),
            ThunderPlugin::System => write!(f, "org.rdk.System"),
            ThunderPlugin::Wifi => write!(f, "org.rdk.Wifi"),
            ThunderPlugin::TextToSpeech => write!(f, "org.rdk.TextToSpeech"),
            ThunderPlugin::Hdcp => write!(f, "org.rdk.HdcpProfile"),
            ThunderPlugin::Telemetry => write!(f, "org.rdk.Telemetry"),
            ThunderPlugin::Analytics => write!(f, "org.rdk.Analytics"),
            ThunderPlugin::UserSettings => write!(f, "org.rdk.UserSettings"),
            ThunderPlugin::RDKShell => write!(f, "org.rdk.RDKShell"),
        }
    }
}

impl ThunderPlugin {
    pub fn cfg(&self) -> Cfg {
        use ThunderPlugin::*;
        match self {
            Controller => CONTROLLER_CFG,
            DeviceInfo => DEVICE_INFO_CFG,
            DisplaySettings => DISPLAY_SETTINGS_CFG,
            HdcpProfile => HDCP_PROFILE_SETTINGS_CFG,
            Hdcp => HDCP_CFG,
            Network => NETWORK_CFG,
            PersistentStorage => PERSISTENT_STORAGE_CFG,
            RDKShell => RDKSHELL_CFG,
            RemoteControl => REMOTE_CONTROL_CFG,
            System => SYSTEM_CFG,
            Wifi => WIFI_CFG,
            LocationSync => LOCATION_SYNC,
            TextToSpeech => TTS_CFG,
            Telemetry => TELEMETRY_CFG,
            Analytics => ANALYTICS_CFG,
            UserSettings => USER_SETTINGS_CFG,
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

    pub fn unversioned_method(&self, method_name: &str) -> String {
        format!("{}.{}", self.callsign(), method_name)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thunder_plugin_controller() {
        // Test ThunderPlugin APIs for Controller
        assert_eq!(ThunderPlugin::Controller.callsign(), "Controller");
        assert_eq!(
            ThunderPlugin::Controller.callsign_and_version(),
            "Controller.1"
        );
        assert_eq!(ThunderPlugin::Controller.callsign_string(), "Controller");
        assert!(!ThunderPlugin::Controller.activate_at_boot());
        assert!(ThunderPlugin::Controller.expect_activated());
        assert_eq!(
            ThunderPlugin::Controller.method("register"),
            "Controller.1.register"
        );
        assert_eq!(
            ThunderPlugin::Controller.method_version("register", 1),
            "Controller.1.register"
        );
        assert_eq!(
            ThunderPlugin::Controller.unversioned_method("register"),
            "Controller.register"
        );
    }
    #[test]
    fn test_thunder_plugin_activates() {
        assert_eq!(ThunderPlugin::activate_on_boot_plugins().len(), 5);
        assert_eq!(ThunderPlugin::expect_activated_plugins().len(), 2);
    }
    #[test]
    fn test_thunder_plugin_config_new() {
        let cfg = ThunderPluginConfig::new("org.test.plugin", true, false);
        assert_eq!(cfg.callsign, "org.test.plugin");
        assert!(cfg.activate_at_boot);
        assert!(!cfg.expect_activated);
    }
    #[test]
    fn test_thunder_plugin_analytics() {
        assert_eq!(ThunderPlugin::Analytics.callsign(), "org.rdk.Analytics");
        assert_eq!(
            ThunderPlugin::Analytics.callsign_and_version(),
            "org.rdk.Analytics.1"
        );
        assert!(!ThunderPlugin::Analytics.activate_at_boot());
        assert!(!ThunderPlugin::Analytics.expect_activated());
    }
}
