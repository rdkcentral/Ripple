use log::{debug, error};

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

pub fn start_service_metrics_timer(extn_client: &ExtnClient, name: String) -> Option<Timer> {
    let metrics_context = &extn_client.get_metrics_context();

    if !metrics_context.enabled {
        return None;
    }

    let metrics_tags = get_metrics_tags(extn_client, InteractionType::Service, None);

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
