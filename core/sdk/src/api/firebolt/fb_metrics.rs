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
//https://developer.comcast.com/firebolt/core/sdk/latest/api/metrics

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
pub struct AppLifecycleStateChange {
    pub app_id: String,
    pub previous_state: Option<AppLifecycleState>,
    pub new_state: AppLifecycleState,
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
}

static FIREBOLT_RPC_NAME: &str = "firebolt_rpc_call";

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

impl std::fmt::Display for MetricsEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricsEnvironment::Prod => write!(f, "prod"),
            MetricsEnvironment::Dev => write!(f, "dev"),
            MetricsEnvironment::Test => write!(f, "test"),
        }
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

#[derive(Debug, PartialEq, PartialOrd, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum FlatMapValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Deserialize, Debug, Clone)]
pub struct ErrorParams {
    #[serde(rename = "type")]
    pub error_type: ErrorType,
    pub code: String,
    pub description: String,
    pub visible: bool,
    pub parameters: Option<HashMap<String, FlatMapValue>>,
    pub age_policy: Option<String>,
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

impl std::fmt::Display for InteractionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InteractionType::Firebolt => write!(f, "fi"),
            InteractionType::Service => write!(f, "si"),
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

#[cfg(test)]
mod tests {
    use super::*;
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
