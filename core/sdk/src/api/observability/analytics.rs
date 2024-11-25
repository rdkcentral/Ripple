use serde::{Deserialize, Serialize};

use crate::{
    api::firebolt::fb_metrics::BehavioralMetricsEvent,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AnalyticsRequest {
    SendMetrics(BehavioralMetricsEvent),
}
#[cfg(test)]
impl Default for AnalyticsRequest {
    fn default() -> Self {
        AnalyticsRequest::SendMetrics(BehavioralMetricsEvent::default())
    }
}
impl ExtnPayloadProvider for AnalyticsRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Analytics(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Analytics(analytics_request)) = payload {
            return Some(analytics_request);
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::Analytics
    }
}
