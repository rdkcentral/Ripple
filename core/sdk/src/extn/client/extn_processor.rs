use crate::{
    extn::{
        extn_capability::ExtnCapability,
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    },
    utils::error::RippleError,
};
use async_trait::async_trait;
use log::{debug, error, trace};
use std::fmt::Debug;
use tokio::sync::mpsc::{self, Receiver as MReceiver, Sender as MSender};

use super::extn_client::ExtnClient;

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

/// ExtnStreamProcessor is a building for any communication receiver within the Ripple IEC
/// A stream processor should
/// 1. Provide a VALUE which is bound by [ExtnPayloadProvider].
/// 2. Have a STATE which is bound by [Clone]
/// 3. Provide a Sender and Receiver
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
    ($get_type:ty, $caller:ident, $recv:ident, $state:ident, $process:ident, $error:ident, $type_check:expr) => {
        let mut rx = $caller.$recv();
        let state = $caller.$state().clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                // check the type of the message
                if $type_check(&msg) {
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
                } else {
                    <$get_type>::$error(
                        state.clone(),
                        msg,
                        $crate::utils::error::RippleError::InvalidInput,
                    )
                    .await;
                }
            }
            drop(rx)
        });
    };
}

/// ExtnRequestProcessor extends [ExtnStreamProcessor] and is the building block for any Request processing within the Ripple IEC.
/// Implementors of ExtnRequestProcessor should implement 2 methods one for Processing request and other for error handling
#[async_trait]
pub trait ExtnRequestProcessor: ExtnStreamProcessor + Send + Sync + 'static {
    /// This method is called for any request which had passed through
    ///
    /// 1. Security checks
    ///
    /// 2. Decoding checks
    ///
    /// 3. Type Validity checks
    ///
    /// So if a given processor recieves this request it is safe and type friendly to implement the processing logic for any given request.
    ///
    /// # Arguments
    ///
    /// `state` -> STATE defined in the [ExtnStreamProcessor]
    ///
    /// `msg` -> Request [ExtnMessage] in the original format useful for responding
    ///
    /// `extracted_message` - VALUE defined in [ExtnStreamProcessor]
    ///
    /// # Returns
    ///
    /// `Option<bool>` -> Used by [ExtnClient] to handle post processing
    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> Option<bool>;

    /// This method is called when there was an error during  processing this incoming request
    /// Erros Could be related to
    ///
    /// 1. Security and Permissions
    ///
    /// 2. Decoding Failures
    ///
    /// 3. Type validity Failures
    ///
    /// Processor should implement the response for such failures.
    async fn process_error(
        state: Self::STATE,
        msg: ExtnMessage,
        error: RippleError,
    ) -> Option<bool>;

    fn check_message_type(message: &ExtnMessage) -> bool {
        message.is_request()
    }

    async fn respond(
        mut extn_client: ExtnClient,
        request: ExtnMessage,
        response: ExtnResponse,
    ) -> Result<(), RippleError> {
        if let Ok(msg) = request.get_response(response) {
            return extn_client.respond(msg).await;
        }
        Err(RippleError::ExtnError)
    }

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
            process_error,
            Self::check_message_type
        );
    }
}

/// ExtnEventProcessor extends [ExtnStreamProcessor] and is the building block for any Event processing within the Ripple IEC.
/// Implementors of ExtnEventProcessor should implement method for processing the event
#[async_trait]
pub trait ExtnEventProcessor: ExtnStreamProcessor + Send + Sync + 'static {
    /// This method is called for any event which had passed through
    ///
    /// 1. Security checks
    ///
    /// 2. Decoding checks
    ///
    /// 3. Type Validity checks
    ///
    /// So if a given processor recieves this event it is safe and type friendly to implement the listening logic for any given event.
    ///
    /// # Arguments
    ///
    /// `state` -> STATE defined in the [ExtnStreamProcessor]
    ///
    /// `msg` -> Request [ExtnMessage] in the original format useful for getting more detail
    ///
    /// `extracted_message` - VALUE defined in [ExtnStreamProcessor]
    ///
    /// # Returns
    ///
    /// `Option<bool>` -> Used by [ExtnClient] to handle post processing
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
            process_error,
            Self::check_message_type
        );
    }

    fn check_message_type(message: &ExtnMessage) -> bool {
        message.is_event()
    }
}
