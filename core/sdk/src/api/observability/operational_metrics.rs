use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum OperationalMetricRequest {
    Subscribe,
    UnSubscribe,
    Counter(Counter),
    Timer(Timer),
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
