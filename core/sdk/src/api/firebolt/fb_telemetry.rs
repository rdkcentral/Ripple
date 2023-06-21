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
use std::collections::HashMap;

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
