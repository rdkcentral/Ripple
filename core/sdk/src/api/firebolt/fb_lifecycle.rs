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

use serde::{Deserialize, Serialize};

use crate::api::apps::CloseReason;

use super::fb_metrics::AppLifecycleState;

pub const LIFECYCLE_EVENT_ON_INACTIVE: &str = "lifecycle.onInactive";
pub const LIFECYCLE_EVENT_ON_FOREGROUND: &str = "lifecycle.onForeground";
pub const LIFECYCLE_EVENT_ON_BACKGROUND: &str = "lifecycle.onBackground";
pub const LIFECYCLE_EVENT_ON_SUSPENDED: &str = "lifecycle.onSuspended";
pub const LIFECYCLE_EVENT_ON_UNLOADING: &str = "lifecycle.onUnloading";

pub const LIFECYCLE_EVENT_ON_START: &str = "lifecycle.onStart";
pub const LIFECYCLE_EVENT_ON_START_SUSPEND: &str = "lifecycle.onStartSuspend";
pub const LIFECYCLE_EVENT_ON_PAUSE: &str = "lifecycle.onPause";
pub const LIFECYCLE_EVENT_ON_ACTIVATE: &str = "lifecycle.onActivate";
pub const LIFECYCLE_EVENT_ON_SUSPEND: &str = "lifecycle.onSuspend";
pub const LIFECYCLE_EVENT_ON_RESUME: &str = "lifecycle.onResume";
pub const LIFECYCLE_EVENT_ON_HIBERNATE: &str = "lifecycle.onHibernate";
pub const LIFECYCLE_EVENT_ON_RESTORE: &str = "lifecycle.onRestore";
pub const LIFECYCLE_EVENT_ON_DESTROY: &str = "lifecycle.onDestroy";

#[derive(Debug, Deserialize, Serialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleState {
    Initializing,
    Inactive,
    Foreground,
    Background,
    Unloading,
    Suspended,
}

impl From<&LifecycleState> for AppLifecycleState {
    fn from(value: &LifecycleState) -> Self {
        match value {
            LifecycleState::Initializing => Self::Initializing,
            LifecycleState::Background => Self::Background,
            LifecycleState::Foreground => Self::Foreground,
            LifecycleState::Inactive => Self::Inactive,
            LifecycleState::Suspended => Self::Suspended,
            LifecycleState::Unloading => Self::NotRunning,
        }
    }
}

impl LifecycleState {
    pub fn as_string(&self) -> &'static str {
        match self {
            LifecycleState::Initializing => "initializing",
            LifecycleState::Inactive => "inactive",
            LifecycleState::Foreground => "foreground",
            LifecycleState::Background => "background",
            LifecycleState::Unloading => "unloading",
            LifecycleState::Suspended => "suspended",
        }
    }

    pub fn as_event(&self) -> &'static str {
        match self {
            LifecycleState::Initializing => "none",
            LifecycleState::Inactive => LIFECYCLE_EVENT_ON_INACTIVE,
            LifecycleState::Foreground => LIFECYCLE_EVENT_ON_FOREGROUND,
            LifecycleState::Background => LIFECYCLE_EVENT_ON_BACKGROUND,
            LifecycleState::Unloading => LIFECYCLE_EVENT_ON_UNLOADING,
            LifecycleState::Suspended => LIFECYCLE_EVENT_ON_SUSPENDED,
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleStateChangeEvent {
    pub app_id: String,
    pub app_instance_id: String,
    pub old_state: LifecycleManagerState,
    pub new_state: LifecycleManagerState,
    //#[serde(skip_serializing_if = "Option::is_none")]
    pub navigation_intent: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CloseRequest {
    pub reason: CloseReason,
}

#[derive(Debug, PartialEq, Serialize, Clone)]
pub enum AppLifecycleState2_0 {
    #[serde(rename = "initializing")]
    Initializing,
    #[serde(rename = "paused")]
    Paused,
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "suspended")]
    Suspended,
    #[serde(rename = "hibernated")]
    Hibernated,
    #[serde(rename = "terminating")]
    Terminating,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct Lifecycle2_0AppEventData {
    pub state: AppLifecycleState2_0,    // The application lifecycle state
    pub previous: AppLifecycleState2_0, // The application lifecycle state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>, // The source of the lifecycle change. "voice" or "remote"
}

#[derive(Debug, Clone)]
pub enum Lifecycle2_0AppEvent {
    OnStart(Lifecycle2_0AppEventData),
    OnStartSuspend(Lifecycle2_0AppEventData),
    OnPause(Lifecycle2_0AppEventData),
    OnActivate(Lifecycle2_0AppEventData),
    OnSuspend(Lifecycle2_0AppEventData),
    OnResume(Lifecycle2_0AppEventData),
    OnHibernate(Lifecycle2_0AppEventData),
    OnRestore(Lifecycle2_0AppEventData),
    OnDestroy(Lifecycle2_0AppEventData),
}

impl Lifecycle2_0AppEvent {
    pub fn as_event_name(&self) -> &'static str {
        match self {
            Lifecycle2_0AppEvent::OnStart(_) => LIFECYCLE_EVENT_ON_START,
            Lifecycle2_0AppEvent::OnStartSuspend(_) => LIFECYCLE_EVENT_ON_START_SUSPEND,
            Lifecycle2_0AppEvent::OnPause(_) => LIFECYCLE_EVENT_ON_PAUSE,
            Lifecycle2_0AppEvent::OnActivate(_) => LIFECYCLE_EVENT_ON_ACTIVATE,
            Lifecycle2_0AppEvent::OnSuspend(_) => LIFECYCLE_EVENT_ON_SUSPEND,
            Lifecycle2_0AppEvent::OnResume(_) => LIFECYCLE_EVENT_ON_RESUME,
            Lifecycle2_0AppEvent::OnHibernate(_) => LIFECYCLE_EVENT_ON_HIBERNATE,
            Lifecycle2_0AppEvent::OnRestore(_) => LIFECYCLE_EVENT_ON_RESTORE,
            Lifecycle2_0AppEvent::OnDestroy(_) => LIFECYCLE_EVENT_ON_DESTROY,
        }
    }
    pub fn as_event_data_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        let data = match self {
            Lifecycle2_0AppEvent::OnStart(d)
            | Lifecycle2_0AppEvent::OnStartSuspend(d)
            | Lifecycle2_0AppEvent::OnPause(d)
            | Lifecycle2_0AppEvent::OnActivate(d)
            | Lifecycle2_0AppEvent::OnSuspend(d)
            | Lifecycle2_0AppEvent::OnResume(d)
            | Lifecycle2_0AppEvent::OnHibernate(d)
            | Lifecycle2_0AppEvent::OnRestore(d)
            | Lifecycle2_0AppEvent::OnDestroy(d) => d,
        };
        serde_json::to_value(data)
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum LifecycleManagerState {
    #[serde(rename = "UNLOADED")]
    Unloaded,
    #[serde(rename = "LOADING")]
    Loading,
    #[serde(rename = "INITIALIZING")]
    Initializing,
    #[serde(rename = "PAUSED")]
    Paused,
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "SUSPENDED")]
    Suspended,
    #[serde(rename = "HIBERNATED")]
    Hibernated,
    #[serde(rename = "TERMINATING")]
    Terminating,
}

impl From<LifecycleManagerState> for AppLifecycleState2_0 {
    fn from(value: LifecycleManagerState) -> Self {
        match value {
            LifecycleManagerState::Initializing => AppLifecycleState2_0::Initializing,
            LifecycleManagerState::Paused => AppLifecycleState2_0::Paused,
            LifecycleManagerState::Active => AppLifecycleState2_0::Active,
            LifecycleManagerState::Suspended => AppLifecycleState2_0::Suspended,
            LifecycleManagerState::Hibernated => AppLifecycleState2_0::Hibernated,
            LifecycleManagerState::Terminating => AppLifecycleState2_0::Terminating,
            _ => AppLifecycleState2_0::Unknown,
        }
    }
}
impl LifecycleManagerState {
    pub fn as_string(&self) -> &'static str {
        match self {
            LifecycleManagerState::Unloaded => "unloaded",
            LifecycleManagerState::Loading => "loading",
            LifecycleManagerState::Initializing => "initializing",
            LifecycleManagerState::Paused => "paused",
            LifecycleManagerState::Active => "active",
            LifecycleManagerState::Suspended => "suspended",
            LifecycleManagerState::Hibernated => "hibernated",
            LifecycleManagerState::Terminating => "terminating",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from() {
        assert_eq!(
            AppLifecycleState::Initializing,
            AppLifecycleState::from(&LifecycleState::Initializing)
        );
        assert_eq!(
            AppLifecycleState::Foreground,
            AppLifecycleState::from(&LifecycleState::Foreground)
        );
        assert_eq!(
            AppLifecycleState::Background,
            AppLifecycleState::from(&LifecycleState::Background)
        );
        assert_eq!(
            AppLifecycleState::Inactive,
            AppLifecycleState::from(&LifecycleState::Inactive)
        );
        assert_eq!(
            AppLifecycleState::Suspended,
            AppLifecycleState::from(&LifecycleState::Suspended)
        );
        assert_eq!(
            AppLifecycleState::NotRunning,
            AppLifecycleState::from(&LifecycleState::Unloading)
        );
    }

    #[test]
    fn test_as_string() {
        assert_eq!("initializing", LifecycleState::Initializing.as_string());
        assert_eq!("inactive", LifecycleState::Inactive.as_string());
        assert_eq!("foreground", LifecycleState::Foreground.as_string());
        assert_eq!("background", LifecycleState::Background.as_string());
        assert_eq!("unloading", LifecycleState::Unloading.as_string());
        assert_eq!("suspended", LifecycleState::Suspended.as_string());
    }

    #[test]
    fn test_as_event() {
        const LIFECYCLE_EVENT_ON_INACTIVE: &str = "lifecycle.onInactive";
        const LIFECYCLE_EVENT_ON_FOREGROUND: &str = "lifecycle.onForeground";
        const LIFECYCLE_EVENT_ON_BACKGROUND: &str = "lifecycle.onBackground";
        const LIFECYCLE_EVENT_ON_UNLOADING: &str = "lifecycle.onUnloading";
        const LIFECYCLE_EVENT_ON_SUSPENDED: &str = "lifecycle.onSuspended";

        assert_eq!("none", LifecycleState::Initializing.as_event());
        assert_eq!(
            LIFECYCLE_EVENT_ON_INACTIVE,
            LifecycleState::Inactive.as_event()
        );
        assert_eq!(
            LIFECYCLE_EVENT_ON_FOREGROUND,
            LifecycleState::Foreground.as_event()
        );
        assert_eq!(
            LIFECYCLE_EVENT_ON_BACKGROUND,
            LifecycleState::Background.as_event()
        );
        assert_eq!(
            LIFECYCLE_EVENT_ON_UNLOADING,
            LifecycleState::Unloading.as_event()
        );
        assert_eq!(
            LIFECYCLE_EVENT_ON_SUSPENDED,
            LifecycleState::Suspended.as_event()
        );
    }
    #[test]
    fn test_lifecycle_state2_0_deserialization() {
        use serde_json::json;

        fn check(s: &str, expected: LifecycleManagerState) {
            let v = json!(s);
            let deserialized: LifecycleManagerState = serde_json::from_value(v).unwrap();
            assert_eq!(deserialized, expected);
        }

        check("LOADING", LifecycleManagerState::Loading);
        check("INITIALIZING", LifecycleManagerState::Initializing);
        check("ACTIVE", LifecycleManagerState::Active);
        check("SUSPENDED", LifecycleManagerState::Suspended);
        check("HIBERNATED", LifecycleManagerState::Hibernated);
        check("TERMINATING", LifecycleManagerState::Terminating);

        // Negative case: should fail to deserialize
        let invalid = serde_json::json!("RUNNING");
        assert!(serde_json::from_value::<LifecycleManagerState>(invalid).is_err());
    }
    #[test]
    fn test_lifecycle_state_change_event_deserialization() {
        use serde_json::json;

        let json_data = json!({
            "appId": "test_app",
            "appInstanceId": "instance_123",
            "oldState": "INITIALIZING",
            "newState": "SUSPENDED",
            "navigationIntent": "some_intent"
        });

        let event: LifecycleStateChangeEvent = serde_json::from_value(json_data).unwrap();

        assert_eq!(event.app_id, "test_app");
        assert_eq!(event.app_instance_id, "instance_123");
        assert_eq!(event.old_state, LifecycleManagerState::Initializing);
        assert_eq!(event.new_state, LifecycleManagerState::Suspended);
        assert_eq!(event.navigation_intent, Some("some_intent".to_string()));
    }
}
