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
    utils::{error::RippleError, mock_utils::PayloadType},
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
                println!("**** extn_processor: start_rx_stream: msg: {:?}", msg);
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
                    println!("**** extn_processor: start_rx_stream: invalid msg: extracted_message: {:?}", extracted_message);
                        continue;
                    }
                    if let Some(v) =
                        <$get_type>::$process(state_c, msg.clone(), extracted_message.unwrap())
                            .await
                    {
                        println!("**** extn_processor: start_rx_stream: v: {:?}", v);
                        if msg.payload.is_event() && v {
                            // trigger closure processor is dropped
                            println!("dropping rx to trigger cleanup");
                            rx.close();
                            break;
                        } else {
                            println!("**** fail 1: Error processing request {:?}", msg);
                        }
                    } else {
                        println!("**** fail 2: Error processing request {:?}", msg);
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
        println!(
            "**** starting request processor for contract {}",
            self.contract().as_clear_string()
        );
        let extn_client = self.get_client();
        let mut receiver = self.receiver();
        let state = self.get_state();
        tokio::spawn(async move {
            while let Some(msg) = receiver.recv().await {
                let extracted_message = Self::get(msg.clone().payload);
                println!(
                    "**** extn_processor: run: extracted_message: {:?}",
                    extracted_message
                );
                if extracted_message.is_none() {
                    Self::handle_error(extn_client.clone(), msg, RippleError::ParseError).await;
                } else if !Self::process_request(
                    state.clone(),
                    msg.clone(),
                    extracted_message.unwrap(),
                )
                .await
                {
                    println!("**** Error processing request {:?}", msg);
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
        println!(
            "**** extn_processor: starting event processor for {:?}",
            self.contract()
        );
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::api::session::{EventAdjective, SessionAdjective};
    use crate::extn::client::extn_client::{tests::Mockable, ExtnClient};
    use crate::extn::client::extn_sender::ExtnSender;
    use crate::extn::extn_client_message::ExtnEvent;
    use crate::extn::extn_id::ExtnId;
    use crate::utils::mock_utils::{get_mock_message, get_mock_request, MockEvent, MockRequest};
    use async_channel::unbounded;
    use std::collections::HashMap;
    use std::sync::Arc;
    use testing_logger::{self, validate};
    use tokio::time::sleep;
    use tokio::time::Duration;

    #[derive(Debug, Clone)]
    pub struct MockState {
        pub(crate) client: ExtnClient,
    }

    impl MockState {
        pub fn get_client(&self) -> ExtnClient {
            self.client.clone()
        }
    }

    fn get_mock_state() -> MockState {
        MockState {
            client: ExtnClient::mock(),
        }
    }

    #[derive(Debug)]
    pub struct MockEventProcessor {
        pub(crate) state: MockState,
        pub(crate) streamer: DefaultExtnStreamer,
    }

    impl Default for MockEventProcessor {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MockEventProcessor {
        pub fn new() -> Self {
            MockEventProcessor {
                state: MockState {
                    client: ExtnClient::mock(),
                },
                streamer: DefaultExtnStreamer::new(),
            }
        }
    }

    #[async_trait]
    impl ExtnStreamProcessor for MockEventProcessor {
        type STATE = MockState;
        type VALUE = MockEvent;

        fn get_state(&self) -> Self::STATE {
            self.state.clone()
        }

        fn receiver(&mut self) -> mpsc::Receiver<ExtnMessage> {
            self.streamer.receiver()
        }

        fn sender(&self) -> mpsc::Sender<ExtnMessage> {
            self.streamer.sender()
        }
    }

    #[async_trait]
    impl ExtnEventProcessor for MockEventProcessor {
        async fn process_event(
            _state: Self::STATE,
            _msg: ExtnMessage,
            _extracted_message: Self::VALUE,
        ) -> Option<bool> {
            println!("**** Success reached process_event");
            Some(true)
        }
    }

    #[derive(Debug)]
    pub struct MockRequestProcessor {
        pub(crate) state: MockState,
        pub(crate) streamer: DefaultExtnStreamer,
    }

    impl Default for MockRequestProcessor {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MockRequestProcessor {
        pub fn new() -> Self {
            MockRequestProcessor {
                state: MockState {
                    client: ExtnClient::mock(),
                },
                streamer: DefaultExtnStreamer::new(),
            }
        }
    }

    #[async_trait]
    impl ExtnStreamProcessor for MockRequestProcessor {
        type STATE = MockState;
        type VALUE = MockRequest;

        fn get_state(&self) -> Self::STATE {
            self.state.clone()
        }

        fn receiver(&mut self) -> mpsc::Receiver<ExtnMessage> {
            self.streamer.receiver()
        }

        fn sender(&self) -> mpsc::Sender<ExtnMessage> {
            self.streamer.sender()
        }

        fn fulfills_mutiple(&self) -> Option<Vec<RippleContract>> {
            Some(vec![
                RippleContract::Internal,
                RippleContract::Session(SessionAdjective::Device),
                RippleContract::DeviceEvents(EventAdjective::Input),
            ])
        }
    }

    #[async_trait]
    impl ExtnRequestProcessor for MockRequestProcessor {
        async fn process_request(state: MockState, msg: ExtnMessage, _val: Self::VALUE) -> bool {
            println!("**** Success reached process_request");
            Self::respond(state.client.clone(), msg, ExtnResponse::Boolean(true))
                .await
                .is_ok()

            // if let Some(ExtnResponse::Boolean(v)) = response.payload.extract() {
            //     assert_eq!(v, true);
            // } else {
            //     assert!(false);
            // }
        }

        fn get_client(&self) -> ExtnClient {
            self.state.client.clone()
        }
    }

    #[tokio::test]
    async fn test_process_request() {
        let extn_client = ExtnClient::mock();
        let state = MockState {
            client: extn_client.clone(),
        };
        let msg = get_mock_message(PayloadType::Request);

        let result =
            MockRequestProcessor::process_request(state, msg.clone(), get_mock_request()).await;
        assert!(result);
    }

    #[tokio::test]
    async fn test_respond() {
        let extn_client = ExtnClient::mock();
        let request = get_mock_message(PayloadType::Request);
        let response = ExtnResponse::Boolean(true);

        let result = MockRequestProcessor::respond(extn_client, request.clone(), response).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ack() {
        let extn_client = ExtnClient::mock();
        let request = get_mock_message(PayloadType::Request);
        let result = MockRequestProcessor::ack(extn_client.clone(), request.clone()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_error() {
        let extn_client = ExtnClient::mock();
        let request = get_mock_message(PayloadType::Request);
        let error = RippleError::ProcessorError;
        let result =
            MockRequestProcessor::handle_error(extn_client.clone(), request.clone(), error.clone())
                .await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_process_event() {
        let processor = MockEventProcessor::new();
        let mock_message = get_mock_message(PayloadType::Event);
        let mock_extracted_message = MockEvent {
            event_name: "test_event".to_string(),
            result: serde_json::json!(true),
            context: None,
            app_id: None,
        };

        let result = MockEventProcessor::process_event(
            processor.get_state(),
            mock_message.clone(),
            mock_extracted_message.clone(),
        )
        .await;

        assert_eq!(result, Some(true));
    }

    #[tokio::test]
    async fn test_get_from_payload() {
        let mock_event = MockEvent {
            event_name: "test_event".to_string(),
            result: serde_json::json!({"key": "value"}),
            context: None,
            app_id: None,
        };

        let extn_payload =
            ExtnPayload::Event(ExtnEvent::Value(serde_json::to_value(mock_event).unwrap()));

        let result = MockEventProcessor::get(extn_payload);

        assert!(result.is_some());
        let extracted_message = result.unwrap();
        assert_eq!(extracted_message.event_name, "test_event");
        assert_eq!(
            extracted_message.result,
            serde_json::json!({"key": "value"})
        );
    }

    #[tokio::test]
    async fn test_get_state() {
        let mock_event_processor = MockEventProcessor::new();
        let state = mock_event_processor.get_state();
        assert!(!state.client.has_internet());
    }

    #[tokio::test]
    async fn test_receiver() {
        let mut mock_event_processor = MockEventProcessor::new();
        let mut receiver = mock_event_processor.receiver();
        let msg = get_mock_message(PayloadType::Event);
        let actual_msg = msg.clone();
        tokio::spawn(async move {
            mock_event_processor
                .sender()
                .send(msg.clone())
                .await
                .unwrap();
        });
        let received_message = receiver.recv().await.expect("Expected a message");
        assert_eq!(received_message.id, actual_msg.id);
        assert_eq!(received_message.requestor, actual_msg.requestor);
    }

    #[tokio::test]
    async fn test_fulfills_multiple() {
        let processor = MockRequestProcessor::new();
        let result = processor.fulfills_mutiple();

        assert_eq!(
            result,
            Some(vec![
                RippleContract::Internal,
                RippleContract::Session(SessionAdjective::Device),
                RippleContract::DeviceEvents(EventAdjective::Input),
            ])
        );
    }

    #[tokio::test]
    async fn test_event_handle_error() {
        let state = MockState {
            client: ExtnClient::mock(),
        };
        let msg = get_mock_message(PayloadType::Event);
        let error = RippleError::ProcessorError;

        let result = MockEventProcessor::handle_error(state, msg, error).await;

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_check_message_type() {
        let event_message = get_mock_message(PayloadType::Event);
        assert!(MockEventProcessor::check_message_type(&event_message));

        let request_message = get_mock_message(PayloadType::Request);
        assert!(!MockEventProcessor::check_message_type(&request_message));
    }

    // #[tokio::test]
    // async fn test_run_with_start_rx_stream_macro() {
    //     let (s, receiver) = tokio::sync::mpsc::unbounded_channel();
    //     let mock_sender = ExtnSender::new(
    //         s,
    //         ExtnId::get_main_target("main".into()),
    //         vec!["context".to_string()],
    //         vec!["fulfills".to_string()],
    //         Some(HashMap::new()),
    //     );
    //     let extn_client = ExtnClient::new(receiver, mock_sender.clone());
    //     let mut processor = MockEventProcessor {
    //         state: MockState {
    //             client: extn_client.clone(),
    //         },
    //         streamer: DefaultExtnStreamer::new(),
    //     };

    //     tokio::spawn(async move {
    //         processor.run().await;
    //     });

    //     tokio::spawn(async move {
    //         tokio::time::sleep(Duration::from_millis(500)).await;

    //         mock_sender
    //             .send(get_mock_message(PayloadType::Event))
    //             .await
    //             .unwrap();
    //     });

    //     tokio::time::sleep(Duration::from_secs(2)).await;

    //     validate(|captured_logs| {
    //         for log in captured_logs {
    //             assert!(log.body.contains("**** extn_processor: start_rx_stream:"));
    //         }
    //     });
    // }

    // #[tokio::test]
    // async fn test_run_with_mock_request_processor() {
    //     testing_logger::setup();
    //     let processor = MockRequestProcessor::new();
    //     let mut processor_for_task = Arc::clone(&processor);

    //     tokio::spawn(async move {
    //         processor_for_task.run().await;
    //     });

    //     // Wait for a short time to allow the asynchronous task to start
    //     tokio::time::sleep(Duration::from_millis(100)).await;

    //     // Simulate sending a message to the processor (this would typically be done in your actual code)
    //     let mock_message = get_mock_message(PayloadType::Request);
    //     processor.sender().send(mock_message.clone()).await.unwrap();

    //     // Wait for a short time to allow the asynchronous task to process the message
    //     tokio::time::sleep(Duration::from_millis(100)).await;

    //     // Assert: Check if the expected log messages were captured
    //     validate(|captured_logs| {
    //         assert!(captured_logs.iter().any(|log| log
    //             .body
    //             .contains("**** extn_processor: run: extracted_message:")));
    //     });
    // }

    // #[tokio::test]
    // async fn test_request_processor_run() {
    //     let extn_client = ExtnClient::mock();
    //     let mut processor = MockRequestProcessor {
    //         state: MockState {
    //             client: extn_client.clone(),
    //         },
    //         streamer: DefaultExtnStreamer::new(),
    //     };
    //     let request_message = get_mock_message(PayloadType::Request);

    //     tokio::spawn(async move {
    //         println!("**** starting request processor green tokio thread for contract internal");
    //         processor.run().await;
    //     });

    //     // let result = processor.sender.send_request(
    //     //     "some_id".to_string(),
    //     //     device_info_request.clone(),
    //     //     Some(sender.tx.clone()),
    //     //     None,
    //     // );

    //     // // Wait for processing to complete
    //     // sleep(Duration::from_secs(2)).await; // Adjust the duration based on your requirements
    // }
}
