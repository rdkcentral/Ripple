use jsonrpsee::core::async_trait;
use ripple_sdk::{framework::bootstrap::Bootstep, utils::error::RippleError};

use crate::{
    processors::thunder_device_info::ThunderDeviceInfoRequestProcessor,
    thunder_state::ThunderBootstrapState,
};

pub struct SetupThunderProcessor;

#[async_trait]
impl Bootstep<ThunderBootstrapState> for SetupThunderProcessor {
    fn get_name(&self) -> String {
        "SetupThunderProcessor".into()
    }

    async fn setup(&self, state: ThunderBootstrapState) -> Result<(), RippleError> {
        state
            .get()
            .get_client()
            .add_request_processor(ThunderDeviceInfoRequestProcessor::new(state.get()));
        Ok(())
    }
}
