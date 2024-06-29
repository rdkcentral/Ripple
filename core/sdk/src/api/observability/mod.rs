use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

use crate::{
    extn::{client::extn_client::ExtnClient, extn_client_message::ExtnResponse},
    utils::error::RippleError,
};

/*
Operational Metrics. These are metrics that are not directly related to the user's behavior. They are
more related to the operation of the platform itself. These metrics are not sent to the BI system, and
are only used for operational/performance measurement - timers, counters, etc -all of low cardinality.
*/

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
impl From<Counter> for OperationalMetricRequest {
    fn from(counter: Counter) -> Self {
        OperationalMetricRequest::Counter(counter)
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

impl From<Timer> for OperationalMetricRequest {
    fn from(timer: Timer) -> Self {
        OperationalMetricRequest::Timer(timer)
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

#[cfg(not(test))]
use log::{debug, error};

#[cfg(test)]
use {println as debug, println as error};

pub fn start_service_metrics_timer(extn_client: &ExtnClient, name: String) -> Option<Timer> {
    let metrics_tags = get_metrics_tags(extn_client, InteractionType::Service, None)?;

    debug!("start_service_metrics_timer: {}: {:?}", name, metrics_tags);

    Some(Timer::start(
        name,
        Some(metrics_tags),
        Some(TimerType::Remote),
    ))
}

pub async fn stop_and_send_service_metrics_timer(
    client: ExtnClient,
    timer: Option<Timer>,
    status: String,
) {
    if let Some(mut timer) = timer {
        timer.stop();
        timer.insert_tag(Tag::Status.key(), status);

        debug!("stop_and_send_service_metrics_timer: {:?}", timer);

        let req = OperationalMetricRequest::Timer(timer);

        let resp: Result<ExtnResponse, RippleError> = client
            .standalone_request(req, SERVICE_METRICS_SEND_REQUEST_TIMEOUT_MS)
            .await;

        if let Err(e) = resp {
            error!(
                "stop_and_send_service_metrics_timer: Failed to send metrics request: e={:?}",
                e
            );
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum OperationalMetricRequest {
    Subscribe,
    UnSubscribe,
    Counter(Counter),
    Timer(Timer),
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum OperationalMetricPayload {
    Timer(Timer),
    Counter(Counter),
}
impl ExtnPayloadProvider for OperationalMetricRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::OperationalMetricsRequest(p)) = payload {
            return Some(p);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::OperationalMetricsRequest(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::Observability
    }

    fn get_contract(&self) -> crate::framework::ripple_contract::RippleContract {
        Self::contract()
    }
}
