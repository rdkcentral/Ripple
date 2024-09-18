use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AnalyticsEvent {
    SendMetrics(Value),
}

impl ExtnPayloadProvider for AnalyticsEvent {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::Analytics(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Event(ExtnEvent::Analytics(analytics_event)) = payload {
            return Some(analytics_event);
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::Analytics
    }
}
