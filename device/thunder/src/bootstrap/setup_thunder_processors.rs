use ripple_sdk::utils::error::RippleError;

use crate::{
    processors::thunder_device_info::ThunderDeviceInfoRequestProcessor,
    thunder_state::ThunderBootstrapStateWithClient,
};

pub struct SetupThunderProcessor;

impl SetupThunderProcessor {
    pub fn get_name() -> String {
        "SetupThunderProcessor".into()
    }

    pub async fn setup(
        state: ThunderBootstrapStateWithClient,
    ) -> Result<ThunderBootstrapStateWithClient, RippleError> {
        state
            .state
            .get_client()
            .add_request_processor(ThunderDeviceInfoRequestProcessor::new(state.state.clone()));
        Ok(state)
    }
}
