use jsonrpsee::core::async_trait;
use ripple_sdk::{framework::bootstrap::Bootstep, utils::error::RippleError};

use crate::{
    processors::thunder_extn_processor::ThunderExtnProcessor, thunder_state::ThunderBootstrapState,
};

pub struct SetupThunderExtns;

#[async_trait]
impl Bootstep<ThunderBootstrapState> for SetupThunderExtns {
    fn get_name(&self) -> String {
        "SetupThunderExtns".into()
    }

    async fn setup(&self, state: ThunderBootstrapState) -> Result<(), RippleError> {
        let extns = state.get_extns();
        let client = state.get().get_client();
        for extn in extns {
            client
                .clone()
                .add_request_processor(ThunderExtnProcessor::new(state.get(), extn));
        }
        Ok(())
    }
}
