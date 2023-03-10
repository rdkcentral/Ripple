use serde::{Deserialize, Serialize};

use crate::api::apps::CloseReason;

pub const LIFECYCLE_EVENT_ON_INACTIVE: &'static str = "lifecycle.onInactive";
pub const LIFECYCLE_EVENT_ON_FOREGROUND: &'static str = "lifecycle.onForeground";
pub const LIFECYCLE_EVENT_ON_BACKGROUND: &'static str = "lifecycle.onBackground";
pub const LIFECYCLE_EVENT_ON_SUSPENDED: &'static str = "lifecycle.onSuspended";
pub const LIFECYCLE_EVENT_ON_UNLOADING: &'static str = "lifecycle.onUnloading";

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
