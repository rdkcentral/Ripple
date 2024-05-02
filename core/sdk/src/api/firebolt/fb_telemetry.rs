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

use crate::{
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider},
    framework::ripple_contract::RippleContract,
};

use super::fb_metrics::{
    Counter, ErrorParams, ErrorType, FlatMapValue, Param, SystemErrorParams, Timer,
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AppLoadStart {
    pub app_id: String,
    pub app_version: Option<String>,
    pub start_time: i64,
    pub ripple_session_id: String,
    pub ripple_version: String,
    pub ripple_context: Option<String>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AppLoadStop {
    pub app_id: String,
    pub stop_time: i64,
    pub ripple_session_id: String,
    pub app_session_id: Option<String>,
    pub success: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AppSDKLoaded {
    pub app_id: String,
    pub stop_time: i64,
    pub ripple_session_id: String,
    pub sdk_name: String,
    pub app_session_id: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct TelemetryAppError {
    pub app_id: String,
    pub error_type: String,
    pub code: String,
    pub description: String,
    pub visible: bool,
    pub parameters: Option<HashMap<String, FlatMapValue>>,
    pub ripple_session_id: String,
}

impl From<ErrorParams> for TelemetryAppError {
    fn from(error: ErrorParams) -> Self {
        TelemetryAppError {
            app_id: String::from(""),
            error_type: get_error_type(error.error_type),
            code: error.code.clone(),
            description: error.description.clone(),
            visible: error.visible,
            parameters: get_params(error.parameters),
            ripple_session_id: String::from(""),
        }
    }
}

fn get_params(error_params: Option<Vec<Param>>) -> Option<HashMap<String, FlatMapValue>> {
    error_params.map(|params| {
        params
            .into_iter()
            .map(|x| (x.name.clone(), x.value))
            .collect::<HashMap<_, _>>()
    })
}

fn get_error_type(error_type: ErrorType) -> String {
    match error_type {
        ErrorType::network => String::from("network"),
        ErrorType::media => String::from("media"),
        ErrorType::restriction => String::from("restriction"),
        ErrorType::entitlement => String::from("entitlement"),
        ErrorType::other => String::from("other"),
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct TelemetrySystemError {
    pub error_name: String,
    pub component: String,
    pub context: Option<String>,
    pub ripple_session_id: String,
    pub ripple_version: String,
    pub ripple_context: Option<String>,
}
impl From<SystemErrorParams> for TelemetrySystemError {
    fn from(error: SystemErrorParams) -> Self {
        TelemetrySystemError {
            error_name: error.error_name,
            component: error.component,
            context: error.context,
            ripple_session_id: String::new(),
            ripple_version: String::from("ripple.version.tbd"), //String::from(version()),
            ripple_context: None,                               //ripple_context(),
        }
    }
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct TelemetrySignIn {
    pub app_id: String,
    pub ripple_session_id: String,
    pub app_session_id: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct TelemetrySignOut {
    pub app_id: String,
    pub ripple_session_id: String,
    pub app_session_id: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct InternalInitialize {
    pub app_id: String,
    pub ripple_session_id: String,
    pub app_session_id: Option<String>,
    pub semantic_version: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct FireboltInteraction {
    pub app_id: String,
    pub method: String,
    pub params: Option<String>,
    pub tt: i64,
    pub success: bool,
    pub ripple_session_id: String,
    pub app_session_id: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum TelemetryPayload {
    AppLoadStart(AppLoadStart),
    AppLoadStop(AppLoadStop),
    AppSDKLoaded(AppSDKLoaded),
    AppError(TelemetryAppError),
    SystemError(TelemetrySystemError),
    SignIn(TelemetrySignIn),
    SignOut(TelemetrySignOut),
    InternalInitialize(InternalInitialize),
    FireboltInteraction(FireboltInteraction), // External Service failures (service, error)
}

impl TelemetryPayload {
    pub fn update_session_id(&mut self, session_id: String) {
        match self {
            Self::AppLoadStart(a) => a.ripple_session_id = session_id,
            Self::AppLoadStop(a) => a.ripple_session_id = session_id,
            Self::AppSDKLoaded(a) => a.ripple_session_id = session_id,
            Self::AppError(a) => a.ripple_session_id = session_id,
            Self::SystemError(s) => s.ripple_session_id = session_id,
            Self::SignIn(s) => s.ripple_session_id = session_id,
            Self::SignOut(s) => s.ripple_session_id = session_id,
            Self::InternalInitialize(i) => i.ripple_session_id = session_id,
            Self::FireboltInteraction(f) => f.ripple_session_id = session_id,
        }
    }
}

impl ExtnPayloadProvider for TelemetryPayload {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::OperationalMetrics(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<TelemetryPayload> {
        if let ExtnPayload::Event(ExtnEvent::OperationalMetrics(r)) = payload {
            return Some(r);
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::TelemetryEventsListener
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum OperationalMetricRequest {
    Subscribe,
    UnSubscribe,
    Counter(Counter),
    Timer(Timer),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_app_error_from_error_params() {
        let error_params = ErrorParams {
            error_type: ErrorType::network,
            code: String::from("123"),
            description: String::from("Network error"),
            visible: true,
            parameters: Some(vec![
                Param {
                    name: String::from("param1"),
                    value: FlatMapValue::String(String::from("value1")),
                },
                Param {
                    name: String::from("param2"),
                    value: FlatMapValue::Boolean(true),
                },
            ]),
        };

        let expected = TelemetryAppError {
            app_id: String::from(""),
            error_type: String::from("network"),
            code: String::from("123"),
            description: String::from("Network error"),
            visible: true,
            parameters: Some(
                vec![
                    (
                        String::from("param1"),
                        FlatMapValue::String(String::from("value1")),
                    ),
                    (String::from("param2"), FlatMapValue::Boolean(true)),
                ]
                .into_iter()
                .collect(),
            ),
            ripple_session_id: String::from(""),
        };

        let result: TelemetryAppError = error_params.into();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_get_params() {
        let error_params = Some(vec![
            Param {
                name: String::from("param1"),
                value: FlatMapValue::String(String::from("value1")),
            },
            Param {
                name: String::from("param2"),
                value: FlatMapValue::Boolean(true),
            },
        ]);

        let expected = Some(
            vec![
                (
                    String::from("param1"),
                    FlatMapValue::String(String::from("value1")),
                ),
                (String::from("param2"), FlatMapValue::Boolean(true)),
            ]
            .into_iter()
            .collect(),
        );

        let result = get_params(error_params);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_get_error_type() {
        assert_eq!(get_error_type(ErrorType::network), String::from("network"));
        assert_eq!(get_error_type(ErrorType::media), String::from("media"));
        assert_eq!(
            get_error_type(ErrorType::restriction),
            String::from("restriction")
        );
        assert_eq!(
            get_error_type(ErrorType::entitlement),
            String::from("entitlement")
        );
        assert_eq!(get_error_type(ErrorType::other), String::from("other"));
    }
}
