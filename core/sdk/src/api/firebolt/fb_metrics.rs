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

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    api::{gateway::rpc_gateway_api::CallContext, session::AccountSession},
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::RippleContract,
};

use super::fb_telemetry::TelemetryPayload;

//https://developer.comcast.com/firebolt/core/sdk/latest/api/metrics

#[derive(Debug, Serialize, Deserialize, Clone)]

pub struct BehavioralMetricContext {
    pub app_id: String,
    pub app_version: String,
    pub partner_id: String,
    pub app_session_id: String,
    pub app_user_session_id: Option<String>,
    pub durable_app_id: String,
    pub governance_state: Option<AppDataGovernanceState>,
}

impl From<CallContext> for BehavioralMetricContext {
    fn from(call_context: CallContext) -> Self {
        BehavioralMetricContext {
            app_id: call_context.app_id.clone(),
            app_version: String::from("app.version.not.implemented"),
            partner_id: String::from("partner.id.not.set"),
            app_session_id: String::from("app_session_id.not.set"),
            durable_app_id: call_context.app_id,
            app_user_session_id: None,
            governance_state: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppDataGovernanceState {
    pub data_tags_to_apply: HashSet<String>,
}
impl AppDataGovernanceState {
    pub fn new(tags: HashSet<String>) -> AppDataGovernanceState {
        AppDataGovernanceState {
            data_tags_to_apply: tags,
        }
    }
}
#[derive(Deserialize, Debug, Clone)]
pub struct InternalInitializeParams {
    pub name: String,
    pub value: Version,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct InternalInitializeResponse {
    pub name: String,
    pub value: SemanticVersion,
}

#[async_trait]
pub trait MetricsContextProvider: core::fmt::Debug {
    async fn provide_context(&mut self) -> Option<MetricsContext>;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Ready {
    pub context: BehavioralMetricContext,
    pub ttmu_ms: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MediaPositionType {
    None,
    PercentageProgress(f32),
    AbsolutePosition(i32),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignIn {
    pub context: BehavioralMetricContext,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SignOut {
    pub context: BehavioralMetricContext,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StartContent {
    pub context: BehavioralMetricContext,
    pub entity_id: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StopContent {
    pub context: BehavioralMetricContext,
    pub entity_id: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Page {
    pub context: BehavioralMetricContext,
    pub page_id: String,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ActionType {
    user,
    app,
}
#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CategoryType {
    user,
    app,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Param {
    pub name: String,
    pub value: String,
}
pub fn hashmap_to_param_vec(the_map: Option<HashMap<String, String>>) -> Vec<Param> {
    let mut result = Vec::new();
    if the_map.is_none() {
        return vec![];
    };

    let params_map = the_map.unwrap();

    for (key, value) in params_map {
        result.push(Param { name: key, value });
    }
    result
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Action {
    pub context: BehavioralMetricContext,
    pub category: CategoryType,
    #[serde(rename = "type")]
    pub _type: String,
    pub parameters: Vec<Param>,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct SemanticVersion {
    pub version: Version,
}

#[derive(Deserialize, Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Version {
    pub major: i8,
    pub minor: i8,
    pub patch: i8,
    pub readable: String,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "foo")
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ErrorType {
    network,
    media,
    restriction,
    entitlement,
    other,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetricsError {
    pub context: BehavioralMetricContext,
    #[serde(alias = "type")]
    pub error_type: ErrorType,
    pub code: String,
    pub description: String,
    pub visible: bool,
    pub parameters: Option<Vec<Param>>,
    pub durable_app_id: String,
    pub third_party_error: bool,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaLoadStart {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaPlay {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaPlaying {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaPause {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaWaiting {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaProgress {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
    pub progress: Option<MediaPositionType>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaSeeking {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
    pub target: Option<MediaPositionType>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaSeeked {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
    pub position: Option<MediaPositionType>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaRateChanged {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
    pub rate: u32,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaRenditionChanged {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
    pub bitrate: u32,
    pub width: u32,
    pub height: u32,
    pub profile: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaEnded {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RawBehaviorMetricRequest {
    pub context: BehavioralMetricContext,
    pub value: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BehavioralMetricPayload {
    Ready(Ready),
    SignIn(SignIn),
    SignOut(SignOut),
    StartContent(StartContent),
    StopContent(StopContent),
    Page(Page),
    Action(Action),
    Error(MetricsError),
    MediaLoadStart(MediaLoadStart),
    MediaPlay(MediaPlay),
    MediaPlaying(MediaPlaying),
    MediaPause(MediaPause),
    MediaWaiting(MediaWaiting),
    MediaProgress(MediaProgress),
    MediaSeeking(MediaSeeking),
    MediaSeeked(MediaSeeked),
    MediaRateChanged(MediaRateChanged),
    MediaRenditionChanged(MediaRenditionChanged),
    MediaEnded(MediaEnded),
    Raw(RawBehaviorMetricRequest),
}

impl BehavioralMetricPayload {
    pub fn update_context(&mut self, context: BehavioralMetricContext) {
        match self {
            Self::Ready(r) => r.context = context,
            Self::SignIn(s) => s.context = context,
            Self::SignOut(s) => s.context = context,
            Self::StartContent(s) => s.context = context,
            Self::StopContent(s) => s.context = context,
            Self::Page(p) => p.context = context,
            Self::Action(a) => a.context = context,
            Self::Error(e) => e.context = context,
            Self::MediaLoadStart(m) => m.context = context,
            Self::MediaPlay(m) => m.context = context,
            Self::MediaPlaying(m) => m.context = context,
            Self::MediaPause(m) => m.context = context,
            Self::MediaWaiting(m) => m.context = context,
            Self::MediaProgress(m) => m.context = context,
            Self::MediaSeeking(m) => m.context = context,
            Self::MediaSeeked(m) => m.context = context,
            Self::MediaRateChanged(m) => m.context = context,
            Self::MediaRenditionChanged(m) => m.context = context,
            Self::MediaEnded(m) => m.context = context,
            Self::Raw(r) => r.context = context,
        }
    }
}

/// all the things that are provided by platform that need to
/// be updated, and eventually in/outjected into/out of a payload
/// These items may (or may not) be available when the ripple
/// process starts, so this service may need a way to wait for the values
/// to become available
/// This design assumes that all of the items will be available at the same times
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MetricsContext {
    pub device_language: String,
    pub device_model: String,
    pub device_id: String,
    pub account_id: String,
    pub device_timezone: String,
    pub device_timezone_offset: String,
    pub device_name: String,
    pub platform: String,
    pub os_ver: String,
    pub distribution_tenant_id: String,
    pub device_session_id: String,
    pub mac_address: String,
    pub serial_number: String,
}

#[allow(non_camel_case_types)]
pub enum MetricsContextField {
    device_language,
    device_model,
    device_id,
    account_id,
    device_timezone,
    device_timezone_offset,
    device_name,
    platform,
    os_ver,
    distributor_id,
    session_id,
    mac_address,
    serial_number,
}
impl MetricsContext {
    pub fn new() -> MetricsContext {
        MetricsContext {
            device_language: String::from(""),
            device_model: String::from(""),
            device_id: String::from(""),
            device_timezone: String::from(""),
            device_timezone_offset: String::from(""),
            device_name: String::from(""),
            mac_address: String::from(""),
            serial_number: String::from(""),
            account_id: String::from(""),
            platform: String::from(""),
            os_ver: String::from(""),
            device_session_id: String::from(""),
            distribution_tenant_id: String::from(""),
        }
    }
    pub fn set(&mut self, field: MetricsContextField, value: String) {
        match field {
            MetricsContextField::device_language => self.device_language = value,
            MetricsContextField::device_model => self.device_model = value,
            MetricsContextField::device_id => self.device_id = value,
            MetricsContextField::account_id => self.account_id = value,
            MetricsContextField::device_timezone => self.device_timezone = value.parse().unwrap(),
            MetricsContextField::device_timezone_offset => {
                self.device_timezone_offset = value.parse().unwrap()
            }
            MetricsContextField::platform => self.platform = value,
            MetricsContextField::os_ver => self.os_ver = value,
            MetricsContextField::distributor_id => self.distribution_tenant_id = value,
            MetricsContextField::session_id => self.device_session_id = value,
            MetricsContextField::mac_address => self.mac_address = value,
            MetricsContextField::serial_number => self.serial_number = value,
            MetricsContextField::device_name => self.device_name = value,
        };
    }
}

#[async_trait]
pub trait BehavioralMetricsService {
    async fn send_metric(&mut self, metrics: BehavioralMetricPayload) -> ();
}
#[async_trait]
pub trait ContextualMetricsService {
    async fn update_metrics_context(&mut self, new_context: Option<MetricsContext>) -> ();
}
#[async_trait]
pub trait MetricsManager: Send + Sync {
    async fn send_metric(&mut self, metrics: BehavioralMetricPayload) -> ();
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BehavioralMetricRequest {
    pub context: Option<MetricsContext>,
    pub payload: BehavioralMetricPayload,
    pub session: AccountSession,
}

impl ExtnPayloadProvider for BehavioralMetricRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::BehavioralMetric(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<BehavioralMetricRequest> {
        if let ExtnPayload::Request(ExtnRequest::BehavioralMetric(r)) = payload {
            return Some(r);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::BehaviorMetrics
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MetricsResponse {
    None,
    Boolean,
}

impl ExtnPayloadProvider for MetricsResponse {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Response(ExtnResponse::Value(value)) = payload {
            if let Ok(v) = serde_json::from_value(value) {
                return Some(v);
            }
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::BehaviorMetrics
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MetricsPayload {
    BehaviorMetric(BehavioralMetricPayload, CallContext),
    OperationalMetric(TelemetryPayload),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetricsRequest {
    pub payload: MetricsPayload,
    /// Additional info extensions want to send which can be appended to the context of the Metrics data
    pub context: Option<HashMap<String, String>>,
}

impl ExtnPayloadProvider for MetricsRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Metrics(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<MetricsRequest> {
        if let ExtnPayload::Request(ExtnRequest::Metrics(r)) = payload {
            return Some(r);
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::Metrics
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ErrorParams {
    #[serde(rename = "type")]
    pub error_type: ErrorType,
    pub code: String,
    pub description: String,
    pub visible: bool,
    pub parameters: Option<Vec<Param>>,
}

impl From<ErrorParams> for ErrorType {
    fn from(params: ErrorParams) -> Self {
        params.error_type
    }
}
