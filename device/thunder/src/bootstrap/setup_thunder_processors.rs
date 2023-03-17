use thunder_ripple_sdk::{
    ripple_sdk::utils::error::RippleError, thunder_state::ThunderBootstrapStateWithClient,
};

use crate::processors::{
    thunder_browser::ThunderBrowserRequestProcessor,
    thunder_device_info::ThunderDeviceInfoRequestProcessor,
    thunder_window_manager::ThunderWindowManagerRequestProcessor,
};

pub struct SetupThunderProcessor;

impl SetupThunderProcessor {
    pub fn get_name() -> String {
        "SetupThunderProcessor".into()
    }

    pub async fn setup(
        state: ThunderBootstrapStateWithClient,
    ) -> Result<ThunderBootstrapStateWithClient, RippleError> {
        let mut extn_client = state.state.get_client();
        extn_client
            .add_request_processor(ThunderDeviceInfoRequestProcessor::new(state.clone().state));
        extn_client.add_request_processor(ThunderBrowserRequestProcessor::new(state.clone().state));
        extn_client.add_request_processor(ThunderWindowManagerRequestProcessor::new(
            state.clone().state,
        ));
        Ok(state)
    }
}
