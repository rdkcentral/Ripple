use crate::{
    api::firebolt::{
        fb_metrics::{
            get_metrics_tags, InteractionType, Tag, Timer, TimerType,
            SERVICE_METRICS_SEND_REQUEST_TIMEOUT_MS,
        },
        fb_telemetry::OperationalMetricRequest,
    },
    extn::{client::extn_client::ExtnClient, extn_client_message::ExtnResponse},
    utils::error::RippleError,
};

use chrono::Utc;
#[cfg(not(test))]
use log::{debug, error};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RpcStats {
    pub start_time: i64,
    pub last_stage: i64,
    stage_durations: String,
}

impl Default for RpcStats {
    fn default() -> Self {
        Self {
            start_time: Utc::now().timestamp_millis(),
            last_stage: 0,
            stage_durations: String::new(),
        }
    }
}

impl RpcStats {
    pub fn update_stage(&mut self, stage: &str) -> i64 {
        let current_time = Utc::now().timestamp_millis();
        let mut last_stage = self.last_stage;
        if last_stage == 0 {
            last_stage = self.start_time;
        }
        self.last_stage = current_time;
        let duration = current_time - last_stage;
        if self.stage_durations.is_empty() {
            self.stage_durations = format!("{}={}", stage, duration);
        } else {
            self.stage_durations = format!("{},{}={}", self.stage_durations, stage, duration);
        }
        duration
    }

    pub fn get_total_time(&self) -> i64 {
        let current_time = Utc::now().timestamp_millis();
        current_time - self.start_time
    }

    pub fn get_stage_durations(&self) -> String {
        self.stage_durations.clone()
    }
}

#[derive(Clone, PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct ApiStats {
    pub api: String,
    pub stats_ref: Option<String>,
    pub stats: RpcStats,
}

impl ApiStats {
    pub fn new(api: String) -> Self {
        Self {
            api,
            stats_ref: None,
            stats: RpcStats::default(),
        }
    }
}

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
        let timer = start_service_metrics_timer(&extn_client, "timer_1".into());
        assert!(timer.is_some(), "Timer should not be None");

        let timer = timer.unwrap();
        assert_eq!(
            timer.name,
            "timer_1".to_string(),
            "Timer name does not match"
        );
        assert_eq!(timer.timer_type, TimerType::Remote);

        let expected_tags = get_metrics_tags(&extn_client, InteractionType::Service, None);
        assert_eq!(
            timer.tags, expected_tags,
            "Timer tags do not match expected tags"
        );
    }
    #[rstest]
    fn test_update_stage() {
        let mut rpc_stats = RpcStats::default();
        let duration = rpc_stats.update_stage("stage1");
        assert!(duration >= 0, "Duration should be non-negative");
        assert_eq!(
            rpc_stats.last_stage,
            Utc::now().timestamp_millis(),
            "Last stage time mismatch"
        );
        assert_eq!(
            rpc_stats.stage_durations,
            "stage1=".to_string() + &duration.to_string(),
            "Stage durations mismatch"
        );

        let duration2 = rpc_stats.update_stage("stage2");
        assert!(duration2 >= 0, "Duration should be non-negative");
        assert_eq!(
            rpc_stats.stage_durations,
            format!("stage1={},stage2={}", duration, duration2),
            "Stage durations mismatch"
        );
    }

    #[rstest]
    fn test_get_total_time() {
        let rpc_stats = RpcStats::default();
        let total_time = rpc_stats.get_total_time();
        assert!(total_time >= 0, "Total time should be non-negative");
    }

    #[rstest]
    fn test_get_stage_durations() {
        let mut rpc_stats = RpcStats::default();
        rpc_stats.update_stage("stage1");
        let stage_durations = rpc_stats.get_stage_durations();
        assert!(
            !stage_durations.is_empty(),
            "Stage durations should not be empty"
        );
    }

    #[rstest]
    fn test_api_stats_new() {
        let api_stats = ApiStats::new("test_api".to_string());
        assert_eq!(api_stats.api, "test_api".to_string(), "API name mismatch");
        assert!(api_stats.stats_ref.is_none(), "Stats ref should be None");
        assert_eq!(
            api_stats.stats.start_time,
            Utc::now().timestamp_millis(),
            "Start time mismatch"
        );
    }
}
