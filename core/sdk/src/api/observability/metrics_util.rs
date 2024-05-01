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
            jv_agent: None,
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
}
