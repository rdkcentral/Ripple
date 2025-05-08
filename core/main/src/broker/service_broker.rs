use crate::state::platform_state::{self, PlatformState};

use super::endpoint_broker::{EndpointBroker, EndpointBrokerState};

pub struct ServiceBroker {
    platform_state: Option<PlatformState>,
}

impl ServiceBroker {
    pub fn new(platform_state: Option<PlatformState>) -> Self {
        Self { platform_state }
    }
}
impl EndpointBroker for ServiceBroker {
    fn get_broker(
        ps: Option<crate::state::platform_state::PlatformState>,
        request: super::endpoint_broker::BrokerConnectRequest,
        callback: super::endpoint_broker::BrokerCallback,
        endpoint_broker: &mut super::endpoint_broker::EndpointBrokerState,
    ) -> Self {
        todo!()
    }

    fn get_sender(&self) -> super::endpoint_broker::BrokerSender {
        todo!()
    }

    fn get_cleaner(&self) -> super::endpoint_broker::BrokerCleaner {
        todo!()
    }
}
