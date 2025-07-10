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
use serde_json::Value;

use crate::{
    client::{
        device_operator::{
            DeviceCallRequest, DeviceChannelParams, DeviceOperator, DeviceResponseMessage,
        },
        thunder_plugin::ThunderPlugin,
    },
    thunder_state::ThunderState,
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct BehavioralMetricsEvent {
    pub event_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_version: Option<String>,
    pub event_source: String,
    pub event_source_version: String,
    pub cet_list: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch_timestamp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_timestamp: Option<u64>,
    pub event_payload: Value,
}

pub async fn send_to_analytics_plugin(
    thunder_state: ThunderState,
    metrics_event: BehavioralMetricsEvent,
) -> DeviceResponseMessage {
    let method: String = ThunderPlugin::Analytics.method("sendEvent");

    match thunder_state
        .get_thunder_client()
        .call(DeviceCallRequest {
            method,
            params: Some(DeviceChannelParams::Json(
                serde_json::to_string(&metrics_event).unwrap(),
            )),
        })
        .await
    {
        Ok(response) => response,
        Err(_) => DeviceResponseMessage {
            message: Value::Null,
            sub_id: None,
        },
    }
}
