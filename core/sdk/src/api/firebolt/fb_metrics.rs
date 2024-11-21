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
    extn::{
        client::extn_client::ExtnClient,
        extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    },
    framework::ripple_contract::RippleContract,
};

use super::fb_telemetry::{OperationalMetricRequest, TelemetryPayload};

//https://developer.comcast.com/firebolt/core/sdk/latest/api/metrics

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
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
    #[serde(deserialize_with = "deserialize_version")]
    pub version: Version,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct InternalInitializeResponse {
    pub version: Version,
}

#[async_trait]
pub trait MetricsContextProvider: core::fmt::Debug {
    async fn provide_context(&mut self) -> Option<MetricsContext>;
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Ready {
    pub context: BehavioralMetricContext,
    pub ttmu_ms: u32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum MediaPositionType {
    None,
    PercentageProgress(f32),
    AbsolutePosition(i32),
}
impl MediaPositionType {
    pub fn as_absolute(self) -> Option<i32> {
        match self {
            MediaPositionType::None => None,
            MediaPositionType::PercentageProgress(_) => None,
            MediaPositionType::AbsolutePosition(absolute) => Some(absolute),
        }
    }
    pub fn as_percentage(self) -> Option<f32> {
        match self {
            MediaPositionType::None => None,
            MediaPositionType::PercentageProgress(percentage) => Some(percentage),
            MediaPositionType::AbsolutePosition(_) => None,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SignIn {
    pub context: BehavioralMetricContext,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SignOut {
    pub context: BehavioralMetricContext,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct StartContent {
    pub context: BehavioralMetricContext,
    pub entity_id: Option<String>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct StopContent {
    pub context: BehavioralMetricContext,
    pub entity_id: Option<String>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum CategoryType {
    user,
    app,
}

#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum FlatMapValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Param {
    pub name: String,
    pub value: FlatMapValue,
}

// custom comparison function used only in unit tests
#[cfg(test)]
impl Param {
    fn cmp(&self, other: &Param) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

pub fn hashmap_to_param_vec(the_map: Option<HashMap<String, FlatMapValue>>) -> Vec<Param> {
    let mut result = Vec::new();

    let params_map = match the_map {
        Some(params_map) => params_map,
        None => return Vec::new(),
    };

    for (key, value) in params_map {
        result.push(Param { name: key, value });
    }

    result
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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

pub fn deserialize_version<'de, D>(deserializer: D) -> Result<Version, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Version::deserialize(deserializer)?;
    let min_value = 0;
    if value.major.ge(&min_value) && value.minor.ge(&min_value) && value.patch.ge(&min_value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "Invalid version should be 0 or greater",
        ))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "foo")
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum ErrorType {
    network,
    media,
    restriction,
    entitlement,
    other,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaLoadStart {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaPlay {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaPlaying {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaPause {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaWaiting {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaProgress {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
    pub progress: Option<MediaPositionType>,
}
impl From<&MediaProgress> for Option<i32> {
    fn from(progress: &MediaProgress) -> Self {
        match progress.progress.clone() {
            Some(prog) => prog.as_absolute(),
            None => None,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaSeeking {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
    pub target: Option<MediaPositionType>,
}
impl From<&MediaSeeking> for Option<i32> {
    fn from(progress: &MediaSeeking) -> Self {
        match progress.target.clone() {
            Some(prog) => prog.as_absolute(),
            None => None,
        }
    }
}

impl From<&MediaSeeking> for Option<f32> {
    fn from(progress: &MediaSeeking) -> Self {
        match progress.target.clone() {
            Some(prog) => prog.as_percentage(),
            None => None,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaSeeked {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
    pub position: Option<MediaPositionType>,
}
impl From<&MediaSeeked> for Option<i32> {
    fn from(progress: &MediaSeeked) -> Self {
        match progress.position.clone() {
            Some(prog) => prog.as_absolute(),
            None => None,
        }
    }
}

impl From<&MediaSeeked> for Option<f32> {
    fn from(progress: &MediaSeeked) -> Self {
        match progress.position.clone() {
            Some(prog) => prog.as_percentage(),
            None => None,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaRateChanged {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
    pub rate: u32,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaRenditionChanged {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
    pub bitrate: u32,
    pub width: u32,
    pub height: u32,
    pub profile: Option<String>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MediaEnded {
    pub context: BehavioralMetricContext,
    pub entity_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct RawBehaviorMetricRequest {
    pub context: BehavioralMetricContext,
    pub value: Value,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AppLifecycleState {
    #[serde(rename = "launching")]
    Launching,
    #[serde(rename = "foreground")]
    Foreground,
    #[serde(rename = "background")]
    Background,
    #[serde(rename = "inactive")]
    Inactive,
    #[serde(rename = "suspended")]
    Suspended,
    #[serde(rename = "not_running")]
    NotRunning,
    #[serde(rename = "initializing")]
    Initializing,
    #[serde(rename = "ready")]
    Ready,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AppLifecycleStateChange {
    pub context: BehavioralMetricContext,
    pub previous_state: Option<AppLifecycleState>,
    pub new_state: AppLifecycleState,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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
    AppStateChange(AppLifecycleStateChange),
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
            Self::AppStateChange(a) => a.context = context,
            Self::Raw(r) => r.context = context,
        }
    }
    pub fn get_context(&self) -> BehavioralMetricContext {
        match self {
            Self::Ready(r) => r.context.clone(),
            Self::SignIn(s) => s.context.clone(),
            Self::SignOut(s) => s.context.clone(),
            Self::StartContent(s) => s.context.clone(),
            Self::StopContent(s) => s.context.clone(),
            Self::Page(p) => p.context.clone(),
            Self::Action(a) => a.context.clone(),
            Self::Error(e) => e.context.clone(),
            Self::MediaLoadStart(m) => m.context.clone(),
            Self::MediaPlay(m) => m.context.clone(),
            Self::MediaPlaying(m) => m.context.clone(),
            Self::MediaPause(m) => m.context.clone(),
            Self::MediaWaiting(m) => m.context.clone(),
            Self::MediaProgress(m) => m.context.clone(),
            Self::MediaSeeking(m) => m.context.clone(),
            Self::MediaSeeked(m) => m.context.clone(),
            Self::MediaRateChanged(m) => m.context.clone(),
            Self::MediaRenditionChanged(m) => m.context.clone(),
            Self::MediaEnded(m) => m.context.clone(),
            Self::AppStateChange(a) => a.context.clone(),
            Self::Raw(r) => r.context.clone(),
        }
    }
}

/*
Operational Metrics. These are metrics that are not directly related to the user's behavior. They are
more related to the operation of the platform itself. These metrics are not sent to the BI system, and
are only used for operational/performance measurement - timers, counters, etc -all of low cardinality.
*/

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Counter {
    pub name: String,
    pub value: u64,
    /*
    TODO... this needs to be a map
    */
    pub tags: Option<HashMap<String, String>>,
}
impl Counter {
    pub fn new(name: String, value: u64, tags: Option<HashMap<String, String>>) -> Counter {
        Counter {
            name: format!("{}_counter", name),
            value,
            tags,
        }
    }
    pub fn increment(&mut self) {
        self.value += 1;
    }
    pub fn decrement(&mut self) {
        self.value -= 1;
    }
    pub fn set_value(&mut self, value: u64) {
        self.value = value;
    }
    pub fn add(&mut self, value: u64) {
        self.value += value;
    }
    pub fn subtract(&mut self, value: u64) {
        self.value -= value;
    }
    pub fn reset(&mut self) {
        self.value = 0;
    }
    pub fn get(&self) -> u64 {
        self.value
    }
    pub fn tag(&mut self, tag_name: String, tag_value: String) {
        if let Some(my_tags) = self.tags.as_mut() {
            my_tags.insert(tag_name, tag_value);
        } else {
            let mut the_map = HashMap::new();
            the_map.insert(tag_name, tag_value);
            self.tags = Some(the_map);
        }
    }
    pub fn error(&mut self) {
        if let Some(my_tags) = self.tags.as_mut() {
            my_tags.insert("error".to_string(), true.to_string());
        }
    }
    pub fn is_error(&self) -> bool {
        if let Some(my_tags) = &self.tags {
            if let Some(error) = my_tags.get("error") {
                error.parse::<bool>().unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        }
    }
    pub fn to_extn_request(&self) -> OperationalMetricRequest {
        OperationalMetricRequest::Counter(self.clone())
    }
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum TimeUnit {
    Nanos,
    Millis,
    Seconds,
}
/*
Type to indicate if the timer is local or remote, used for bucket sizing in downstreams
*/
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum TimerType {
    /*
    Local metrics are generated for local, on device service calls
    */
    Local,
    /*
    Remote metrics are generated for remote, off device service calls
    */
    Remote,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Timer {
    pub name: String,
    #[serde(with = "serde_millis")]
    pub start: std::time::Instant,
    #[serde(with = "serde_millis")]
    pub stop: Option<std::time::Instant>,
    pub tags: Option<HashMap<String, String>>,
    pub time_unit: TimeUnit,
    pub timer_type: TimerType,
}
impl Timer {
    pub fn start(
        name: String,
        tags: Option<HashMap<String, String>>,
        timer_type: Option<TimerType>,
    ) -> Timer {
        Timer::new(
            name,
            std::time::Instant::now(),
            tags,
            TimeUnit::Millis,
            timer_type,
        )
    }
    pub fn start_with_time_unit(
        name: String,
        tags: Option<HashMap<String, String>>,
        time_unit: TimeUnit,
        timer_type: Option<TimerType>,
    ) -> Timer {
        Timer::new(name, std::time::Instant::now(), tags, time_unit, timer_type)
    }

    pub fn new(
        name: String,
        start: std::time::Instant,
        tags: Option<HashMap<String, String>>,
        time_unit: TimeUnit,
        timer_type: Option<TimerType>,
    ) -> Timer {
        Timer {
            name,
            start,
            stop: None,
            tags,
            time_unit,
            /*most will probably be local, so default as such*/
            timer_type: timer_type.unwrap_or(TimerType::Local),
        }
    }

    pub fn stop(&mut self) -> Timer {
        if self.stop.is_none() {
            self.stop = Some(std::time::Instant::now());
        }
        self.clone()
    }

    pub fn restart(&mut self) {
        self.start = std::time::Instant::now();
        self.stop = None;
    }

    pub fn elapsed(&self) -> std::time::Duration {
        match self.stop {
            Some(stop) => stop.duration_since(self.start),
            None => self.start.elapsed(),
        }
    }

    pub fn insert_tag(&mut self, tag_name: String, tag_value: String) {
        if let Some(my_tags) = self.tags.as_mut() {
            my_tags.insert(tag_name, tag_value);
        }
    }

    pub fn insert_tags(&mut self, new_tags: HashMap<String, String>) {
        let tags = self.tags.clone();
        match tags {
            Some(mut t) => t.extend(new_tags),
            None => self.tags = Some(new_tags),
        }
    }

    pub fn error(&mut self) {
        if let Some(my_tags) = self.tags.as_mut() {
            my_tags.insert("error".to_string(), true.to_string());
        }
    }

    pub fn to_extn_request(&self) -> OperationalMetricRequest {
        OperationalMetricRequest::Timer(self.clone())
    }
}

static FIREBOLT_RPC_NAME: &str = "firebolt_rpc_call";
impl From<Timer> for OperationalMetricRequest {
    fn from(timer: Timer) -> Self {
        OperationalMetricRequest::Timer(timer)
    }
}

pub fn fb_api_counter(method_name: String, tags: Option<HashMap<String, String>>) -> Counter {
    let counter_tags = match tags {
        Some(mut tags) => {
            tags.insert("method_name".to_string(), method_name);
            tags
        }
        None => {
            let mut the_map = HashMap::new();
            the_map.insert("method_name".to_string(), method_name);
            the_map
        }
    };
    Counter {
        name: FIREBOLT_RPC_NAME.to_string(),
        value: 1,
        tags: Some(counter_tags),
    }
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum OperationalMetricPayload {
    Timer(Timer),
    Counter(Counter),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum MetricsEnvironment {
    Prod,
    Dev,
    Test,
}

impl Default for MetricsEnvironment {
    fn default() -> Self {
        Self::Prod
    }
}

impl ToString for MetricsEnvironment {
    fn to_string(&self) -> String {
        match self {
            MetricsEnvironment::Prod => "prod".into(),
            MetricsEnvironment::Dev => "dev".into(),
            MetricsEnvironment::Test => "test".into(),
        }
    }
}

/// all the things that are provided by platform that need to
/// be updated, and eventually in/outjected into/out of a payload
/// These items may (or may not) be available when the ripple
/// process starts, so this service may need a way to wait for the values
/// to become available
/// This design assumes that all of the items will be available at the same times
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct MetricsContext {
    pub enabled: bool,
    pub device_language: String,
    pub device_model: String,
    pub device_id: Option<String>,
    pub account_id: Option<String>,
    pub device_timezone: String,
    pub device_timezone_offset: String,
    pub device_name: Option<String>,
    pub platform: String,
    pub os_name: String,
    pub os_ver: String,
    pub distribution_tenant_id: String,
    pub device_session_id: String,
    pub mac_address: String,
    pub serial_number: String,
    pub firmware: String,
    pub ripple_version: String,
    pub env: Option<String>,
    pub data_governance_tags: Option<Vec<String>>,
    pub activated: Option<bool>,
    pub proposition: String,
    pub retailer: Option<String>,
    pub primary_provider: Option<String>,
    pub coam: Option<bool>,
    pub country: Option<String>,
    pub region: Option<String>,
    pub account_type: Option<String>,
    pub operator: Option<String>,
    pub account_detail_type: Option<String>,
    pub device_type: String,
    pub device_manufacturer: String,
    pub authenticated: Option<bool>,
}

#[allow(non_camel_case_types)]
pub enum MetricsContextField {
    enabled,
    device_language,
    device_model,
    device_id,
    account_id,
    device_timezone,
    device_timezone_offset,
    device_name,
    platform,
    os_name,
    os_ver,
    distributor_id,
    session_id,
    mac_address,
    serial_number,
    firmware,
    ripple_version,
    env,
    cet_list,
    activated,
    proposition,
    retailer,
    primary_provider,
    coam,
    country,
    region,
    account_type,
    operator,
    account_detail_type,
}

impl MetricsContext {
    pub fn new() -> MetricsContext {
        MetricsContext {
            enabled: false,
            device_language: String::from(""),
            device_model: String::from(""),
            device_id: None,
            device_timezone: String::from(""),
            device_timezone_offset: String::from(""),
            device_name: None,
            mac_address: String::from(""),
            serial_number: String::from(""),
            account_id: None,
            platform: String::from("entos:rdk"),
            os_name: String::from(""),
            os_ver: String::from(""),
            device_session_id: String::from(""),
            distribution_tenant_id: String::from(""),
            firmware: String::from(""),
            ripple_version: String::from(""),
            env: None,
            data_governance_tags: None,
            activated: None,
            proposition: String::from(""),
            retailer: None,
            primary_provider: None,
            coam: None,
            country: None,
            region: None,
            account_type: None,
            operator: None,
            account_detail_type: None,
            device_type: String::from(""),
            device_manufacturer: String::from(""),
            authenticated: None,
        }
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
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum MetricsPayload {
    BehaviorMetric(BehavioralMetricPayload, CallContext),
    TelemetryPayload(TelemetryPayload),
    OperationalMetric(OperationalMetricPayload),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
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

#[derive(Debug, Clone)]
pub struct SystemErrorParams {
    pub error_name: String,
    pub component: String,
    pub context: Option<String>,
}

pub const SERVICE_METRICS_SEND_REQUEST_TIMEOUT_MS: u64 = 2000;

pub enum InteractionType {
    Firebolt,
    Service,
}

impl ToString for InteractionType {
    fn to_string(&self) -> String {
        match self {
            InteractionType::Firebolt => "fi".into(),
            InteractionType::Service => "si".into(),
        }
    }
}

pub enum Tag {
    Type,
    App,
    AppVersion,
    Firmware,
    Status,
    RippleVersion,
    Features,
}

impl Tag {
    pub fn key(&self) -> String {
        match self {
            Tag::Type => "type".into(),
            Tag::App => "app".into(),
            Tag::AppVersion => "app_version".into(),
            Tag::Firmware => "firmware".into(),
            Tag::Status => "status".into(),
            Tag::RippleVersion => "ripple".into(),
            Tag::Features => "features".into(),
        }
    }
}

pub fn get_metrics_tags(
    extn_client: &ExtnClient,
    interaction_type: InteractionType,
    app_id: Option<String>,
) -> Option<HashMap<String, String>> {
    let metrics_context = extn_client.get_metrics_context()?;
    let mut tags: HashMap<String, String> = HashMap::new();

    tags.insert(Tag::Type.key(), interaction_type.to_string());

    if let Some(app) = app_id {
        tags.insert(Tag::App.key(), app);
    }

    tags.insert(Tag::Firmware.key(), metrics_context.firmware.clone());
    tags.insert(Tag::RippleVersion.key(), metrics_context.ripple_version);

    let features = extn_client.get_features();
    let feature_count = features.len();
    let mut features_str = String::new();

    if feature_count > 0 {
        for (i, feature) in features.iter().enumerate() {
            features_str.push_str(feature);
            if i < feature_count - 1 {
                features_str.push(',');
            }
        }
    }

    tags.insert(Tag::Features.key(), features_str);

    Some(tags)
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::gateway::rpc_gateway_api::ApiProtocol;
    use crate::utils::test_utils::test_extn_payload_provider;
    use serde_json::json;

    #[test]
    fn test_initialize_params() {
        let value = json!({ "version":{"major": 0,"minor": 13,"patch": 0,"readable": "Firebolt Core SDK 0.13.0"}} );
        assert!(InternalInitializeParams::deserialize(value).is_ok());
        let value = json!({ "version":{"major": -1,"minor": 13,"patch": 0,"readable": "Firebolt Core SDK 0.13.0"}} );
        assert!(InternalInitializeParams::deserialize(value).is_err());
        let value = json!({ "version":{"major": 0,"minor": -2,"patch": 0,"readable": "Firebolt Core SDK 0.13.0"}} );
        assert!(InternalInitializeParams::deserialize(value).is_err());
        let value = json!({ "version":{"major": 0,"minor": 0,"patch": -3,"readable": "Firebolt Core SDK 0.13.0"}} );
        assert!(InternalInitializeParams::deserialize(value).is_err());
        let value = json!({ "version":{"major": 0,"minor": 13,"patch": 0,"readable": 1}} );
        assert!(InternalInitializeParams::deserialize(value).is_err());
    }

    #[test]
    pub fn test_counter() {
        let mut counter = super::Counter::new("test".to_string(), 0, None);
        counter.increment();
        assert_eq!(counter.get(), 1);
        counter.decrement();
        assert_eq!(counter.get(), 0);
        counter.set_value(10);
        assert_eq!(counter.get(), 10);
        counter.add(5);
        assert_eq!(counter.get(), 15);
        counter.subtract(5);
        assert_eq!(counter.get(), 10);
        counter.reset();
        assert_eq!(counter.get(), 0);
    }
    #[test]
    pub fn test_counter_with_tags() {
        let mut counter = super::Counter::new("test".to_string(), 0, None);
        assert_eq!(counter.tags, None);
        counter.tag("tag1".to_string(), "tag1_value".to_string());
        counter.tag("tag2".to_string(), "tag2_value".to_string());
        let mut expected = HashMap::new();
        expected.insert("tag1".to_string(), "tag1_value".to_string());
        expected.insert("tag2".to_string(), "tag2_value".to_string());
        assert_eq!(counter.tags, Some(expected));
    }
    #[test]
    pub fn test_timer() {
        let mut timer = super::Timer::start("test".to_string(), None, None);
        std::thread::sleep(std::time::Duration::from_millis(101));
        timer.stop();
        assert!(timer.elapsed().as_millis() > 100);
        assert!(timer.elapsed().as_millis() < 200);
        timer.restart();
        std::thread::sleep(std::time::Duration::from_millis(101));
        timer.stop();
        assert!(timer.elapsed().as_millis() > 100);
        assert!(timer.elapsed().as_millis() < 200);
    }

    #[test]
    fn test_extn_request_behavioral_metric() {
        let behavioral_metric_context = BehavioralMetricContext {
            app_id: "test_app_id".to_string(),
            app_version: "test_app_version".to_string(),
            partner_id: "test_partner_id".to_string(),
            app_session_id: "test_app_session_id".to_string(),
            app_user_session_id: Some("test_user_session_id".to_string()),
            durable_app_id: "test_durable_app_id".to_string(),
            governance_state: Some(AppDataGovernanceState {
                data_tags_to_apply: HashSet::new(),
            }),
        };

        let ready_payload = Ready {
            context: behavioral_metric_context,
            ttmu_ms: 100,
        };

        let behavioral_metric_request = BehavioralMetricRequest {
            context: Some(MetricsContext {
                enabled: true,
                device_language: "en".to_string(),
                device_model: "iPhone".to_string(),
                device_id: Some("test_device_id".to_string()),
                account_id: Some("test_account_id".to_string()),
                device_timezone: "GMT".to_string(),
                device_timezone_offset: "+0:00".to_string(),
                device_name: Some("TestDevice".to_string()),
                platform: "iOS".to_string(),
                os_name: "test_os_name".to_string(),
                os_ver: "14.0".to_string(),
                distribution_tenant_id: "test_distribution_tenant_id".to_string(),
                device_session_id: "test_device_session_id".to_string(),
                mac_address: "test_mac_address".to_string(),
                serial_number: "test_serial_number".to_string(),
                firmware: "test_firmware".to_string(),
                ripple_version: "test_ripple_version".to_string(),
                env: Some("test_env".to_string()),
                data_governance_tags: None,
                activated: None,
                proposition: "test_proposition".to_string(),
                retailer: None,
                primary_provider: None,
                coam: None,
                country: None,
                region: None,
                account_type: None,
                operator: None,
                account_detail_type: None,
                device_type: "test_device_type".to_string(),
                device_manufacturer: "test_device_manufacturer".to_string(),
                authenticated: None,
            }),
            payload: BehavioralMetricPayload::Ready(ready_payload),
            session: AccountSession {
                id: "test_session_id".to_string(),
                token: "test_token".to_string(),
                account_id: "test_account_id".to_string(),
                device_id: "test_device_id".to_string(),
            },
        };

        let contract_type: RippleContract = RippleContract::BehaviorMetrics;
        test_extn_payload_provider(behavioral_metric_request, contract_type);
    }

    #[test]
    fn test_extn_request_metrics() {
        let behavior_metric_payload = BehavioralMetricPayload::Ready(Ready {
            context: BehavioralMetricContext {
                app_id: "test_app_id".to_string(),
                app_version: "test_app_version".to_string(),
                partner_id: "test_partner_id".to_string(),
                app_session_id: "test_app_session_id".to_string(),
                app_user_session_id: Some("test_user_session_id".to_string()),
                durable_app_id: "test_durable_app_id".to_string(),
                governance_state: Some(AppDataGovernanceState {
                    data_tags_to_apply: HashSet::new(),
                }),
            },
            ttmu_ms: 100,
        });
        let call_context = CallContext {
            session_id: "test_session_id".to_string(),
            request_id: "test_request_id".to_string(),
            app_id: "test_app_id".to_string(),
            call_id: 123,
            protocol: ApiProtocol::Extn,
            method: "some method".to_string(),
            cid: Some("test_cid".to_string()),
            gateway_secure: true,
            context: Vec::new(),
        };

        let metrics_request = MetricsRequest {
            payload: MetricsPayload::BehaviorMetric(behavior_metric_payload, call_context),
            context: None,
        };

        let contract_type: RippleContract = RippleContract::Metrics;
        test_extn_payload_provider(metrics_request, contract_type);
    }

    #[test]
    fn test_extn_response_metrics() {
        let metrics_response = MetricsResponse::None;
        let contract_type: RippleContract = RippleContract::BehaviorMetrics;
        test_extn_payload_provider(metrics_response, contract_type);
    }

    #[test]
    fn test_hashmap_to_param_vec() {
        let mut map = HashMap::new();
        map.insert(
            "key1".to_string(),
            FlatMapValue::String("value1".to_string()),
        );
        map.insert("key2".to_string(), FlatMapValue::Number(2.0));
        map.insert("key3".to_string(), FlatMapValue::Boolean(true));
        let mut vec = hashmap_to_param_vec(Some(map));
        let mut expected = vec![
            Param {
                name: "key1".to_string(),
                value: FlatMapValue::String("value1".to_string()),
            },
            Param {
                name: "key2".to_string(),
                value: FlatMapValue::Number(2.0),
            },
            Param {
                name: "key3".to_string(),
                value: FlatMapValue::Boolean(true),
            },
        ];
        vec.sort_by(|param1, param2| param1.cmp(param2));
        expected.sort_by(|param1, param2| param1.cmp(param2));
        assert_eq!(vec, expected);
    }

    #[test]
    fn test_flatmap() {
        let flatmap = FlatMapValue::String("value1".to_string());
        let flatmap2 = FlatMapValue::Number(2.0);
        let flatmap3 = FlatMapValue::Boolean(true);
        assert_eq!(flatmap, FlatMapValue::String("value1".to_string()));
        assert_eq!(flatmap2, FlatMapValue::Number(2.0));
        assert_eq!(flatmap3, FlatMapValue::Boolean(true));
    }

    #[test]
    fn test_behavioral_metric_payload() {
        let behavioral_metric_context = BehavioralMetricContext {
            app_id: "test_app_id".to_string(),
            app_version: "test_app_version".to_string(),
            partner_id: "test_partner_id".to_string(),
            app_session_id: "test_app_session_id".to_string(),
            app_user_session_id: Some("test_user_session_id".to_string()),
            durable_app_id: "test_durable_app_id".to_string(),
            governance_state: Some(AppDataGovernanceState {
                data_tags_to_apply: HashSet::new(),
            }),
        };

        let ready_payload = Ready {
            context: behavioral_metric_context.clone(),
            ttmu_ms: 100,
        };

        let mut behavioral_metric_payload = BehavioralMetricPayload::Ready(ready_payload);

        assert_eq!(
            behavioral_metric_payload.get_context(),
            behavioral_metric_context
        );

        let new_behavioral_metric_context = BehavioralMetricContext {
            app_id: "new_test_app_id".to_string(),
            app_version: "new_test_app_version".to_string(),
            partner_id: "new_test_partner_id".to_string(),
            app_session_id: "new_test_app_session_id".to_string(),
            app_user_session_id: Some("new_test_user_session_id".to_string()),
            durable_app_id: "new_test_durable_app_id".to_string(),
            governance_state: Some(AppDataGovernanceState {
                data_tags_to_apply: HashSet::new(),
            }),
        };

        behavioral_metric_payload.update_context(new_behavioral_metric_context.clone());

        assert_eq!(
            behavioral_metric_payload.get_context(),
            new_behavioral_metric_context
        );
    }

    #[test]
    fn test_timer_start() {
        let timer = Timer::start("test_timer".to_string(), None, None);
        assert_eq!(timer.name, "test_timer");
        assert!(timer.start.elapsed().as_millis() < 10);
        assert_eq!(timer.stop, None);
        assert_eq!(timer.tags, None);
        assert_eq!(timer.time_unit, TimeUnit::Millis);
        assert_eq!(timer.timer_type, TimerType::Local);
    }

    #[test]
    fn test_timer_start_with_time_unit() {
        let timer =
            Timer::start_with_time_unit("test_timer".to_string(), None, TimeUnit::Seconds, None);
        assert_eq!(timer.name, "test_timer");
        assert!(timer.start.elapsed().as_secs() < 1);
        assert_eq!(timer.stop, None);
        assert_eq!(timer.tags, None);
        assert_eq!(timer.time_unit, TimeUnit::Seconds);
        assert_eq!(timer.timer_type, TimerType::Local);
    }

    #[test]
    fn test_timer_stop() {
        let mut timer = Timer::start("test_timer".to_string(), None, None);
        std::thread::sleep(std::time::Duration::from_secs(1));
        let stopped_timer = timer.stop();
        assert_eq!(stopped_timer.name, "test_timer");
        assert!(stopped_timer.elapsed().as_secs() >= 1);
        assert_eq!(stopped_timer.tags, None);
        assert_eq!(stopped_timer.time_unit, TimeUnit::Millis);
        assert_eq!(stopped_timer.timer_type, TimerType::Local);
    }

    #[test]
    fn test_timer_restart() {
        let mut timer = Timer::start("test_timer".to_string(), None, None);
        std::thread::sleep(std::time::Duration::from_secs(1));
        timer.restart();
        assert_eq!(timer.name, "test_timer");
        assert!(timer.start.elapsed().as_secs() < 1);
        assert_eq!(timer.stop, None);
        assert_eq!(timer.tags, None);
        assert_eq!(timer.time_unit, TimeUnit::Millis);
        assert_eq!(timer.timer_type, TimerType::Local);
    }

    #[test]
    fn test_insert_tag() {
        let mut timer = Timer::new(
            "test_timer".to_string(),
            std::time::Instant::now(),
            Some(
                vec![("tag1".to_string(), "value1".to_string())]
                    .into_iter()
                    .collect(),
            ),
            TimeUnit::Millis,
            None,
        );

        timer.insert_tag("tag2".to_string(), "value2".to_string());

        assert_eq!(
            timer.tags,
            Some(
                vec![
                    ("tag1".to_string(), "value1".to_string()),
                    ("tag2".to_string(), "value2".to_string())
                ]
                .into_iter()
                .collect()
            )
        );
    }

    #[test]
    fn test_timer_insert_tags() {
        let mut timer = Timer::start("test_timer".to_string(), None, None);
        let mut new_tags = HashMap::new();
        new_tags.insert("tag_name".to_string(), "tag_value".to_string());
        timer.insert_tags(new_tags);
        assert_eq!(
            timer.tags.unwrap().get("tag_name"),
            Some(&"tag_value".to_string())
        );
    }

    #[test]
    fn test_timer_error() {
        let mut timer = Timer::new(
            "test_timer".to_string(),
            std::time::Instant::now(),
            Some(HashMap::new()),
            TimeUnit::Millis,
            None,
        );
        timer.error();
        assert_eq!(timer.tags.unwrap().get("error"), Some(&"true".to_string()));
    }

    #[test]
    fn test_timer_to_extn_request() {
        let timer = Timer::start("test_timer".to_string(), None, None);
        let request = timer.to_extn_request();
        assert_eq!(request, OperationalMetricRequest::Timer(timer));
    }

    #[test]
    fn test_fb_api_counter() {
        let counter = fb_api_counter("test_method".to_string(), None);
        assert_eq!(counter.name, "firebolt_rpc_call");
        assert_eq!(counter.value, 1);
        assert_eq!(
            counter.tags.unwrap().get("method_name"),
            Some(&"test_method".to_string())
        );
    }
}
