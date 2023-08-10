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

pub const LIFECYCLE_EVENT_ON_INACTIVE: &str = "lifecycle.onInactive";
pub const LIFECYCLE_EVENT_ON_FOREGROUND: &str = "lifecycle.onForeground";
pub const LIFECYCLE_EVENT_ON_BACKGROUND: &str = "lifecycle.onBackground";
pub const LIFECYCLE_EVENT_ON_SUSPENDED: &str = "lifecycle.onSuspended";
pub const LIFECYCLE_EVENT_ON_UNLOADING: &str = "lifecycle.onUnloading";

#[derive(Debug, Deserialize, Serialize, Copy, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleState {
    Initializing,
    Inactive,
    Foreground,
    Background,
    Unloading,
    Suspended,
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
