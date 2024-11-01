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
}
