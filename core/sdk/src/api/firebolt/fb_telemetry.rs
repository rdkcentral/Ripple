use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppLoadStart {
    pub app_id: String,
    pub app_version: Option<String>,
    pub start_time: i64,
    pub ripple_session_id: String,
    pub ripple_version: String,
    pub ripple_context: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppLoadStop {
    pub app_id: String,
    pub stop_time: i64,
    pub ripple_session_id: String,
    pub app_session_id: Option<String>,
    pub success: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSDKLoaded {
    pub app_id: String,
    pub stop_time: i64,
    pub ripple_session_id: String,
    pub sdk_name: String,
    pub app_session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppError {
    pub app_id: String,
    pub error_type: String,
    pub code: String,
    pub description: String,
    pub visible: bool,
    pub parameters: Option<HashMap<String, String>>,
    pub ripple_session_id: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemError {
    pub error_name: String,
    pub component: String,
    pub context: Option<String>,
    pub ripple_session_id: String,
    pub ripple_version: String,
    pub ripple_context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignIn {
    pub app_id: String,
    pub ripple_session_id: String,
    pub app_session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignOut {
    pub app_id: String,
    pub ripple_session_id: String,
    pub app_session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InternalInitialize {
    pub app_id: String,
    pub ripple_session_id: String,
    pub app_session_id: Option<String>,
    pub semantic_version: String,
}

// #[derive(Debug, Serialize, Deserialize, Clone)]
// pub enum TelemetryRequest {
//     AppLoadStart(AppLoadStart),
//     AppLoadStop(AppLoadStop),
//     AppSDKLoaded(AppSDKLoaded),
//     AppError(AppError),
//     SystemError(SystemError),
//     SignIn(SignIn),
//     SignOut(SignOut),
//     InternalInitialize(InternalInitialize),
// }
