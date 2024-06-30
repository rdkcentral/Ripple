use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
};

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
    pub status: MetricStatus,
}
impl Counter {
    pub fn new(name: String, value: u64, tags: Option<HashMap<String, String>>) -> Counter {
        Counter {
            name: format!("{}_counter", name),
            value,
            tags,
            status: MetricStatus::Unknown,
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
        self.status = MetricStatus::Failure;
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[repr(i32)]
pub enum MetricStatus {
    Error,
    Failure,
    Success,
    Unset,
    Unknown,
    PermissionError(i32),
    ServiceError(i32),
}
impl From<Option<MetricStatus>> for MetricStatus {
    fn from(status: Option<MetricStatus>) -> Self {
        status.unwrap_or(MetricStatus::Unset)
    }
}
impl MetricStatus {
    /*
    could not get ordinals to work with type coercion, so just doing it manually
    */
    pub fn ordinal(&self) -> i32 {
        match self {
            MetricStatus::Error => -2,
            MetricStatus::Failure => -1,
            MetricStatus::Success => 0,
            MetricStatus::Unset => 1,
            MetricStatus::Unknown => 2,
            MetricStatus::PermissionError(code) => *code,
            MetricStatus::ServiceError(code) => *code,
        }
    }
}

impl Display for MetricStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.ordinal())
    }
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
    pub status: MetricStatus,
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
            status: MetricStatus::Unset,
        }
    }

    pub fn stop(&mut self) -> Timer {
        if self.stop.is_none() {
            self.stop = Some(std::time::Instant::now());
        }
        if self.status != MetricStatus::Unset {
            self.insert_tag(Tag::Status.key(), self.status.to_string());
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
        let mut tags = HashMap::new();

        tags.insert(tag_name, tag_value);
        self.insert_tags(tags);
    }

    pub fn insert_tags(&mut self, new_tags: HashMap<String, String>) {
        match self.tags.as_mut() {
            Some(t) => t.extend(new_tags),
            None => self.tags = Some(new_tags),
        }
    }

    pub fn error(&mut self) {
        self.status = MetricStatus::Error;
        if let Some(my_tags) = self.tags.as_mut() {
            my_tags.insert("error".to_string(), true.to_string());
        }
    }
    pub fn set_status(&mut self, status: MetricStatus) {
        self.status = status;
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
    status: Option<MetricStatus>,
) {
    if let Some(mut timer) = timer {
        timer.set_status(status.into());
        timer.stop();

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
pub async fn emit_observability(client: ExtnClient, timers: Vec<Timer>, counters: Vec<Counter>) {
    for mut timer in timers {
        timer.stop();
        timer.insert_tag(Tag::Status.key(), timer.status.to_string());
        let resp: Result<ExtnResponse, RippleError> = client
            .standalone_request(
                OperationalMetricRequest::Timer(timer),
                SERVICE_METRICS_SEND_REQUEST_TIMEOUT_MS,
            )
            .await;
        if let Err(e) = resp {
            error!(
                "stop_and_send_service_metrics_timer: Failed to send metrics request: e={:?}",
                e
            );
        }
    }
    for counter in counters {
        let resp: Result<ExtnResponse, RippleError> = client
            .standalone_request(
                OperationalMetricRequest::Counter(counter),
                SERVICE_METRICS_SEND_REQUEST_TIMEOUT_MS,
            )
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{context::RippleContextUpdateRequest, firebolt::fb_metrics::MetricsContext};
    use crate::extn::client::extn_client::tests::Mockable;
    use rstest::rstest;

    fn get_mock_metrics_context() -> MetricsContext {
        MetricsContext {
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
        }
    }

    #[rstest]
    fn test_start_service_metrics_timer() {
        let extn_client = ExtnClient::mock();
        let request = RippleContextUpdateRequest::MetricsContext(get_mock_metrics_context());
        extn_client.context_update(request);
        let timer = start_service_metrics_timer(&extn_client, "package_manager_get_list".into());
        assert!(timer.is_some(), "Timer should not be None");

        let timer = timer.unwrap();
        assert_eq!(
            timer.name,
            "package_manager_get_list".to_string(),
            "Timer name does not match"
        );
        assert_eq!(timer.timer_type, TimerType::Remote);

        let expected_tags = get_metrics_tags(&extn_client, InteractionType::Service, None);
        assert_eq!(
            timer.tags, expected_tags,
            "Timer tags do not match expected tags"
        );
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
        let mut timer = Timer::start("test".to_string(), None, None);
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
}
