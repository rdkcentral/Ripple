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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum LifecycleState2_0 {
    #[serde(rename = "LOADING")]
    Loading,
    #[serde(rename = "INITIALIZING")]
    Initializing,
    #[serde(rename = "RUNREQUESTED")]
    RunRequested,
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "ACTIVATEREQUESTED")]
    ActivateRequested,
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "DEACTIVATEREQUESTED")]
    DeactivateRequested,
    #[serde(rename = "SUSPENDREQUESTED")]
    SuspendRequested,
    #[serde(rename = "SUSPENDED")]
    Suspended,
    #[serde(rename = "RESUMEREQUESTED")]
    ResumeRequested,
    #[serde(rename = "HIBERNATEREQUESTED")]
    HibernateRequested,
    #[serde(rename = "HIBERNATED")]
    Hibernated,
    #[serde(rename = "WAKEREQUESTED")]
    WakeRequested,
    #[serde(rename = "TERMINATEREQUESTED")]
    TerminateRequested,
    #[serde(rename = "TERMINATING")]
    Terminating,
}

impl LifecycleState2_0 {
    pub fn as_string(&self) -> &'static str {
        match self {
            LifecycleState2_0::Loading => "LOADING",
            LifecycleState2_0::Initializing => "INITIALIZING",
            LifecycleState2_0::RunRequested => "RUNREQUESTED",
            LifecycleState2_0::Running => "RUNNING",
            LifecycleState2_0::ActivateRequested => "ACTIVATEREQUESTED",
            LifecycleState2_0::Active => "ACTIVE",
            LifecycleState2_0::DeactivateRequested => "DEACTIVATEREQUESTED",
            LifecycleState2_0::SuspendRequested => "SUSPENDREQUESTED",
            LifecycleState2_0::Suspended => "SUSPENDED",
            LifecycleState2_0::ResumeRequested => "RESUMEREQUESTED",
            LifecycleState2_0::HibernateRequested => "HIBERNATEREQUESTED",
            LifecycleState2_0::Hibernated => "HIBERNATED",
            LifecycleState2_0::WakeRequested => "WAKEREQUESTED",
            LifecycleState2_0::TerminateRequested => "TERMINATEREQUESTED",
            LifecycleState2_0::Terminating => "TERMINATING",
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleStateChangeEvent {
    pub app_id: String,
    pub app_instance_id: String,
    pub old_state: LifecycleState2_0,
    pub new_state: LifecycleState2_0,
    //#[serde(skip_serializing_if = "Option::is_none")]
    pub navigation_intent: Option<String>,
}
#[derive(Deserialize, Debug, Clone)]
pub struct CloseRequest {
    pub reason: CloseReason,
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

        fn check(s: &str, expected: LifecycleState2_0) {
            let v = json!(s);
            let deserialized: LifecycleState2_0 = serde_json::from_value(v).unwrap();
            assert_eq!(deserialized, expected);
        }

        check("LOADING", LifecycleState2_0::Loading);
        check("INITIALIZING", LifecycleState2_0::Initializing);
        check("RUNREQUESTED", LifecycleState2_0::RunRequested);
        check("RUNNING", LifecycleState2_0::Running);
        check("ACTIVATEREQUESTED", LifecycleState2_0::ActivateRequested);
        check("ACTIVE", LifecycleState2_0::Active);
        check(
            "DEACTIVATEREQUESTED",
            LifecycleState2_0::DeactivateRequested,
        );
        check("SUSPENDREQUESTED", LifecycleState2_0::SuspendRequested);
        check("SUSPENDED", LifecycleState2_0::Suspended);
        check("RESUMEREQUESTED", LifecycleState2_0::ResumeRequested);
        check("HIBERNATEREQUESTED", LifecycleState2_0::HibernateRequested);
        check("HIBERNATED", LifecycleState2_0::Hibernated);
        check("WAKEREQUESTED", LifecycleState2_0::WakeRequested);
        check("TERMINATEREQUESTED", LifecycleState2_0::TerminateRequested);
        check("TERMINATING", LifecycleState2_0::Terminating);

        // Negative case: should fail to deserialize
        let invalid = serde_json::json!("RUN_REQUESTED");
        assert!(serde_json::from_value::<LifecycleState2_0>(invalid).is_err());
    }
    #[test]
    fn test_lifecycle_state_change_event_deserialization() {
        use serde_json::json;

        let json_data = json!({
            "appId": "test_app",
            "appInstanceId": "instance_123",
            "oldState": "RUNNING",
            "newState": "SUSPENDED",
            "navigationIntent": "some_intent"
        });

        let event: LifecycleStateChangeEvent = serde_json::from_value(json_data).unwrap();

        assert_eq!(event.app_id, "test_app");
        assert_eq!(event.app_instance_id, "instance_123");
        assert_eq!(event.old_state, LifecycleState2_0::Running);
        assert_eq!(event.new_state, LifecycleState2_0::Suspended);
        assert_eq!(event.navigation_intent, Some("some_intent".to_string()));
    }
}
