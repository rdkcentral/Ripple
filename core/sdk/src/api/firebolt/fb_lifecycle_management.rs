use serde::{Deserialize, Serialize};

use crate::api::apps::CloseReason;

use super::fb_discovery::NavigationIntent;

pub const LCM_EVENT_ON_REQUEST_READY: &'static str = "lifecyclemanagement.onRequestReady";
pub const LCM_EVENT_ON_REQUEST_CLOSE: &'static str = "lifecyclemanagement.onRequestClose";
pub const LCM_EVENT_ON_REQUEST_FINISHED: &'static str = "lifecyclemanagement.onRequestFinished";
pub const LCM_EVENT_ON_REQUEST_LAUNCH: &'static str = "lifecyclemanagement.onRequestLaunch";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum LifecycleManagementRequest {
    Launch(LifecycleManagementLaunchRequest),
    Ready(LifecycleManagementReadyRequest),
    Close(LifecycleManagementCloseRequest),
    Finished(LifecycleManagementFinishedRequest),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LifecycleManagementLaunchRequest {
    pub parameters: LifecycleManagementLaunchParameters,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementLaunchParameters {
    pub app_id: String,
    pub intent: Option<NavigationIntent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LifecycleManagementReadyRequest {
    pub parameters: LifecycleManagementReadyParameters,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementReadyParameters {
    pub app_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LifecycleManagementCloseRequest {
    pub parameters: LifecycleManagementCloseParameters,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementCloseParameters {
    pub app_id: String,
    pub reason: CloseReason,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementFinishedRequest {
    pub parameters: LifecycleManagementFinishedParameters,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleManagementFinishedParameters {
    pub app_id: String,
}
