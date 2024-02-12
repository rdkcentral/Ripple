use crate::{
    api::firebolt::fb_telemetry::OperationalMetricRequest,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

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
    use crate::{
        api::firebolt::fb_telemetry::{AppLoadStart, TelemetryPayload},
        utils::test_utils::test_extn_payload_provider,
    };

    #[test]
    fn test_extn_request_operational_metric() {
        let operational_metric_request = OperationalMetricRequest::Subscribe;
        let contract_type: RippleContract = RippleContract::OperationalMetricListener;
        test_extn_payload_provider(operational_metric_request, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_telemetry_payload() {
        let app_load_start_payload = AppLoadStart {
            app_id: "example_app".to_string(),
            app_version: Some("1.0.0".to_string()),
            start_time: 1634816400,
            ripple_session_id: "session_id".to_string(),
            ripple_version: "1.2.3".to_string(),
            ripple_context: Some("context_data".to_string()),
        };
        let telemetry_payload = TelemetryPayload::AppLoadStart(app_load_start_payload);
        let contract_type: RippleContract = RippleContract::OperationalMetricListener;
        test_extn_payload_provider(telemetry_payload, contract_type);
    }
}
