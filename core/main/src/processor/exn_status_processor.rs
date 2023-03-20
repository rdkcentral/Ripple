use ripple_sdk::{
    api::status_update::ExtnStatus,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnEventProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    log::error,
    tokio::sync::{mpsc::Receiver as MReceiver, mpsc::Sender as MSender},
};

use crate::state::extn_state::ExtnState;

#[derive(Debug)]
pub struct ExtnStatusProcessor {
    state: ExtnState,
    streamer: DefaultExtnStreamer,
}

/// Event processor used for cases where a certain Extension Capability is required to be ready.
/// Bootstrap uses the [WaitForStatusReadyEventProcessor] to await during Device Connnection before starting the gateway.
impl ExtnStatusProcessor {
    pub fn new(state: ExtnState) -> ExtnStatusProcessor {
        ExtnStatusProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for ExtnStatusProcessor {
    type VALUE = ExtnStatus;
    type STATE = ExtnState;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnEventProcessor for ExtnStatusProcessor {
    async fn process_event(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool> {
        let id = msg.requestor.clone();
        state.update_extn_status(id.clone(), extracted_message.clone());
        if let Some(v) = state.get_extn_status_listener(id.clone()) {
            if let Err(e) = v.send(extracted_message.clone()).await {
                error!("Error while sending status {:?}", e);
            }
        }
        None
    }
}
