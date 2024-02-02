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
