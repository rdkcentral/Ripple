use crate::{
    extn::{
        extn_capability::ExtnCapability,
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnPayloadProvider},
    },
    utils::error::RippleError,
};
use async_trait::async_trait;
use log::{debug, error, trace};
use std::fmt::Debug;
use tokio::sync::mpsc::{self, Receiver as MReceiver, Sender as MSender};

/// ExtnRequestHandler is a building block for a any request handler within the Ripple Ecosystem
/// A Request handler should
/// 1. Provide a type which is bounded by the ExtnPayloadProvider. Check `ExtnPayloadProvider for more information about what
/// 2. Have a handle method which takes in a Result from the transaction
/// 3. Provide a Unique Uuid for the the transaction

#[derive(Debug)]
pub struct DefaultExtnStreamer {
    rx: Option<MReceiver<ExtnMessage>>,
    tx: Option<MSender<ExtnMessage>>,
}

impl DefaultExtnStreamer {
    pub fn new() -> DefaultExtnStreamer {
        let (tx, rx) = mpsc::channel(10);
        DefaultExtnStreamer {
            rx: Some(rx),
            tx: Some(tx),
        }
    }
}

impl ExtnStreamer for DefaultExtnStreamer {
    fn sender(&self) -> MSender<ExtnMessage> {
        self.tx.clone().unwrap()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        let rx = self.rx.take();
        rx.unwrap()
    }
}

pub trait ExtnStreamer {
    fn sender(&self) -> MSender<ExtnMessage>;
    fn receiver(&mut self) -> MReceiver<ExtnMessage>;
}

pub trait ExtnStreamProcessor: Send + Sync + 'static {
    type VALUE: ExtnPayloadProvider;
    type STATE: Clone + Send + Sync;
    fn get(payload: ExtnPayload) -> Option<Self::VALUE> {
        Self::VALUE::get_from_payload(payload)
    }

    fn get_state(&self) -> Self::STATE;
    fn receiver(&mut self) -> MReceiver<ExtnMessage>;
    fn capability(&self) -> ExtnCapability {
        Self::VALUE::cap()
    }
    fn sender(&self) -> MSender<ExtnMessage>;
}

#[macro_export]
macro_rules! start_rx_stream {
    ($get_type:ty, $caller:ident, $recv:ident, $state:ident, $process:ident, $error:ident) => {
        let mut rx = $caller.$recv();
        let state = $caller.$state().clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let state_c = state.clone();
                let extracted_message = <$get_type>::get(msg.payload.clone());
                if extracted_message.is_none() {
                    <$get_type>::$error(
                        state_c,
                        msg,
                        $crate::utils::error::RippleError::ParseError,
                    )
                    .await;
                    continue;
                }
                if let Some(v) =
                    <$get_type>::$process(state_c, msg, extracted_message.unwrap()).await
                {
                    if v {
                        // trigger closure processor is dropped
                        trace!("dropping rx to trigger cleanup");
                        rx.close();
                        break;
                    }
                }
            }
            drop(rx)
        });
    };
}

#[async_trait]
pub trait ExtnRequestProcessor: ExtnStreamProcessor + Send + Sync + 'static {
    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool>;

    async fn process_error(
        state: Self::STATE,
        msg: ExtnMessage,
        error: RippleError,
    ) -> Option<bool>;

    async fn run(&mut self) {
        debug!(
            "starting request processor for {}",
            self.capability().to_string()
        );
        start_rx_stream!(
            Self,
            self,
            receiver,
            get_state,
            process_request,
            process_error
        );
    }
}

#[async_trait]
pub trait ExtnEventProcessor: ExtnStreamProcessor + Send + Sync + 'static {
    async fn process_event(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool>;

    async fn process_error(
        _state: Self::STATE,
        msg: ExtnMessage,
        error: RippleError,
    ) -> Option<bool> {
        error!("invalid event received {:?} for {:?}", msg.payload, error);
        None
    }

    async fn run(&mut self) {
        debug!(
            "starting event processor for {}",
            self.capability().to_string()
        );
        start_rx_stream!(
            Self,
            self,
            receiver,
            get_state,
            process_event,
            process_error
        );
    }
}
