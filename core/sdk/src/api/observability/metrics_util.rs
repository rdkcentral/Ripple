use log::error;
use tonic::{Code, Response, Status};

use crate::{
    api::firebolt::fb_metrics::{
        get_metrics_tags, InteractionType, MetricsContext, MetricsPayload, MetricsRequest,
        OperationalMetricPayload, Tag, Timer, SERVICE_METRICS_SEND_REQUEST_TIMEOUT_MS,
    },
    extn::{client::extn_client::ExtnClient, extn_client_message::ExtnResponse},
    utils::error::RippleError,
};

pub fn start_service_metrics_timer(
    extn_client: &ExtnClient,
    metrics_context: &MetricsContext,
    name: String,
) -> Timer {
    let metrics_tags =
        get_metrics_tags(extn_client, metrics_context, InteractionType::Service, None);

    Timer::start(
        name,
        metrics_context.device_session_id.clone(),
        Some(metrics_tags),
    )
}

pub async fn stop_and_send_metrics_timer(mut client: ExtnClient, mut timer: Timer, status: String) {
    timer.stop();
    timer.insert_tag(Tag::Status.key(), status);

    println!("*** _DEBUG: stop_and_send_metrics_timer: {:?}", timer);

    let req = MetricsRequest {
        payload: MetricsPayload::OperationalMetric(OperationalMetricPayload::Timer(timer)),
        context: None,
    };

    let resp: Result<ExtnResponse, RippleError> = client
        .standalone_request(req, SERVICE_METRICS_SEND_REQUEST_TIMEOUT_MS)
        .await;

    if let Err(e) = resp {
        error!(
            "stop_and_send_metrics_timer: Failed to send metrics request: e={:?}",
            e
        );
    }
}
