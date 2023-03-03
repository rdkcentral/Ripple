use ripple_sdk::framework::RippleResponse;

use crate::{
    processors::thunder_extn_processor::ThunderExtnProcessor,
    thunder_state::ThunderBootstrapStateWithClient,
};

pub struct SetupThunderExtns;

impl SetupThunderExtns {
    pub fn get_name() -> String {
        "SetupThunderExtns".into()
    }

    pub async fn setup(state: ThunderBootstrapStateWithClient) -> RippleResponse {
        let extns = state.prev.prev.extns;
        let thunder_state = state.state;
        for extn in extns {
            thunder_state
                .get_client()
                .clone()
                .add_request_processor(ThunderExtnProcessor::new(thunder_state.clone(), extn));
        }
        Ok(())
    }
}
