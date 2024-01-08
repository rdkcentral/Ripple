// Copyright 2023 Comcast Cable Communications Management, LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{
    extn::extn_client_message::{ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    framework::{ripple_contract::RippleContract, RippleResponse},
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

impl Default for DefaultExtnStreamer {
    fn default() -> Self {
        Self::new()
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
    fn fulfills_mutiple(&self) -> Option<Vec<RippleContract>> {
        None
    }
    fn contract(&self) -> RippleContract {
        Self::VALUE::contract()
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
                        <$get_type>::$process(state_c, msg.clone(), extracted_message.unwrap())
                            .await
                    {
                        if msg.payload.is_event() && v {
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
    /// None - means not processed
    /// Some(true) - Successful processing with status success
    /// Some(false) - Successful processing with status error
    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool;

    /// For [ExtnRequestProcessor] each implementor should return an instance of [ExtnClient]
    /// This is necessary for the processor to intenally log and delegate errors.
    fn get_client(&self) -> ExtnClient;

    fn check_message_type(message: &ExtnMessage) -> bool {
        message.payload.is_request()
    }

    async fn respond(
        mut extn_client: ExtnClient,
        request: ExtnMessage,
        response: ExtnResponse,
    ) -> RippleResponse {
        if let Ok(msg) = request.get_response(response) {
            return extn_client.send_message(msg).await;
        }
        Err(RippleError::ExtnError)
    }

    async fn ack(mut extn_client: ExtnClient, request: ExtnMessage) -> RippleResponse {
        return extn_client.send_message(request.ack()).await;
    }

    async fn run(&mut self) {
        debug!(
            "starting request processor for contract {}",
            self.contract().as_clear_string()
        );
        let extn_client = self.get_client();
        let mut receiver = self.receiver();
        let state = self.get_state();
        tokio::spawn(async move {
            while let Some(msg) = receiver.recv().await {
                let extracted_message = Self::get(msg.clone().payload);
                if extracted_message.is_none() {
                    Self::handle_error(extn_client.clone(), msg, RippleError::ParseError).await;
                } else if !Self::process_request(
                    state.clone(),
                    msg.clone(),
                    extracted_message.unwrap(),
                )
                .await
                {
                    debug!("Error processing request {:?}", msg);
                }
            }
        });
    }

    async fn handle_error(
        mut extn_client: ExtnClient,
        req: ExtnMessage,
        error: RippleError,
    ) -> bool {
        if let Err(e) = extn_client.respond(req, ExtnResponse::Error(error)).await {
            error!("Error during responding {:?}", e);
        }
        // to support return chaining in processors
        false
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

    async fn handle_error(
        _state: Self::STATE,
        msg: ExtnMessage,
        error: RippleError,
    ) -> Option<bool> {
        error!("invalid event received {:?} for {:?}", msg.payload, error);
        None
    }

    async fn run(&mut self) {
        debug!("starting event processor for {:?}", self.contract());
        start_rx_stream!(
            Self,
            self,
            receiver,
            get_state,
            process_event,
            handle_error,
            Self::check_message_type
        );
    }

    fn check_message_type(message: &ExtnMessage) -> bool {
        message.payload.is_event()
    }
}
