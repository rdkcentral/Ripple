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

use super::extn_client::ExtnClient;
use crate::{
    extn::extn_client_message::{ExtnMessage, ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    framework::{ripple_contract::RippleContract, RippleResponse},
    utils::error::RippleError,
};
use async_trait::async_trait;
#[cfg(not(test))]
use log::{debug, error, trace};
use std::fmt::Debug;
use tokio::sync::mpsc::{self, Receiver as MReceiver, Sender as MSender};

#[cfg(test)]
use {println as trace, println as debug, println as error};

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

#[derive(Debug, Clone, Default)]
pub struct Prerequisites {
    pub need_internet: bool,
    pub need_activation: bool,
}

impl Prerequisites {
    pub fn new(need_internet: bool, need_activation: bool) -> Self {
        Prerequisites {
            need_internet,
            need_activation,
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
    type VALUE: ExtnPayloadProvider + Debug;
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
    fn get_prerequisites(&self) -> Prerequisites {
        Prerequisites::default()
    }
}

#[macro_export]
macro_rules! start_rx_stream {
    ($get_type:ty, $caller:ident, $recv:ident, $state:ident, $process:ident, $error:ident, $type_check:expr) => {
        let mut rx = $caller.$recv();
        let state = $caller.$state().clone();
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
                    <$get_type>::$process(state_c, msg.clone(), extracted_message.unwrap()).await
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
    /// `bool` -> Used by [ExtnClient] to handle post processing
    /// `true` - Successful processing with status success
    /// `false` - Successful processing with status error
    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool;

    /// For [ExtnRequestProcessor] each implementor should return an instance of [ExtnClient]
    /// This is necessary for the processor to intenally log and delegate errors.
    fn get_client(&self) -> ExtnClient;

    fn needs_internet(&self) -> bool {
        false
    }

    fn has_internet(&self) -> bool {
        self.get_client().has_internet()
    }

    fn check_prerequisties(prereq: &Prerequisites, client: &ExtnClient) -> bool {
        match (prereq.need_internet, client.has_internet()) {
            (true, true) | (false, _) => true,
            (true, false) => false,
        }
    }

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
        extn_client.send_message(request.ack()).await
    }

    async fn run(&mut self) {
        debug!(
            "starting request processor for contract {}",
            self.contract().as_clear_string()
        );
        let extn_client = self.get_client();
        let mut receiver = self.receiver();
        let state = self.get_state();
        let prereq = self.get_prerequisites();
        tokio::spawn(async move {
            while let Some(msg) = receiver.recv().await {
                let extracted_message = Self::get(msg.clone().payload);
                if extracted_message.is_none() {
                    Self::handle_error(extn_client.clone(), msg, RippleError::ParseError).await;
                } else if !Self::check_prerequisties(&prereq, &extn_client) {
                    error!(
                        "Prerequsties not statisfied: {:?}. Not processing request: {:?} by ",
                        prereq, extracted_message
                    );
                    Self::handle_error(extn_client.clone(), msg, RippleError::ProcessorError).await;
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::api::manifest::extn_manifest::ExtnSymbol;
    use crate::api::session::{EventAdjective, SessionAdjective};
    use crate::extn::client::extn_client::{tests::Mockable, ExtnClient};
    use crate::extn::client::extn_sender::{tests::Mockable as extn_sender_mockable, ExtnSender};
    use crate::extn::extn_client_message::ExtnEvent;
    use crate::extn::extn_id::ExtnId;
    use crate::utils::mock_utils::{
        get_mock_message, get_mock_request, MockEvent, MockRequest, PayloadType,
    };
    use chrono::Utc;
    use log::info;
    use rstest::rstest;
    use uuid::Uuid;

    #[derive(Debug, Clone)]
    pub struct MockState {
        pub(crate) client: ExtnClient,
        pub(crate) contracts: Vec<RippleContract>,
    }

    impl MockState {
        pub fn get_client(&self) -> ExtnClient {
            self.client.clone()
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
        pub fn new_v1(client: ExtnClient, contracts: Vec<RippleContract>) -> Self {
            MockEventProcessor {
                state: MockState { client, contracts },
                streamer: DefaultExtnStreamer::new(),
            }
        }

        pub fn new() -> Self {
            MockEventProcessor {
                state: MockState {
                    client: ExtnClient::mock(),
                    contracts: Vec::new(),
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
            extracted_message: Self::VALUE,
        ) -> Option<bool> {
            info!("processing event {:?}", extracted_message);
            if let Some(ExtnResponse::Boolean(value)) = extracted_message.expected_response {
                Some(value)
            } else {
                None
            }
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
        pub fn new_v1(client: ExtnClient, contracts: Vec<RippleContract>) -> Self {
            MockRequestProcessor {
                state: MockState { client, contracts },
                streamer: DefaultExtnStreamer::new(),
            }
        }

        pub fn new() -> Self {
            MockRequestProcessor {
                state: MockState {
                    client: ExtnClient::mock(),
                    contracts: vec![
                        RippleContract::Internal,
                        RippleContract::Session(SessionAdjective::Device),
                        RippleContract::DeviceEvents(EventAdjective::Input),
                    ],
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
            Some(self.state.contracts.clone())
        }
    }

    #[async_trait]
    impl ExtnRequestProcessor for MockRequestProcessor {
        async fn process_request(state: MockState, msg: ExtnMessage, val: Self::VALUE) -> bool {
            let expected_response = val.expected_response;

            if let Some(resp) = expected_response {
                info!("**** process_request: msg: {:?}, resp: {:?}", msg, resp);
                Self::respond(state.client.clone(), msg, resp).await.is_ok()
            } else {
                // Handle the case when expected_response is None
                // For example, use a default response or return an error
                false
            }
        }

        fn get_client(&self) -> ExtnClient {
            self.state.client.clone()
        }
    }

    #[tokio::test]
    async fn test_process_request() {
        let extn_client = ExtnClient::mock();
        let msg = get_mock_message(PayloadType::Request);

        let result = MockRequestProcessor::process_request(
            MockState {
                client: extn_client,
                contracts: vec![RippleContract::Internal],
            },
            msg.clone(),
            get_mock_request(),
        )
        .await;
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
            expected_response: Some(ExtnResponse::Boolean(true)),
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
            expected_response: Some(ExtnResponse::Boolean(true)),
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
            contracts: vec![RippleContract::Internal],
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

    #[rstest(exp_resp, case(Some(ExtnResponse::Boolean(true))))]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_request_processor_run(exp_resp: Option<ExtnResponse>) {
        let (mock_sender, receiver) = ExtnSender::mock();
        let mut extn_client = ExtnClient::new(receiver, mock_sender.clone());
        let processor = MockRequestProcessor {
            state: MockState {
                client: extn_client.clone(),
                contracts: vec![RippleContract::Internal],
            },
            streamer: DefaultExtnStreamer::new(),
        };

        extn_client.add_request_processor(processor);
        let extn_client_for_thread = extn_client.clone();

        tokio::spawn(async move {
            extn_client_for_thread.clone().initialize().await;
        });

        let response = extn_client
            .request(MockRequest {
                app_id: "test_app_id".to_string(),
                contract: RippleContract::Internal,
                expected_response: exp_resp.clone(),
            })
            .await;

        match response {
            Ok(actual_response) => {
                let expected_message = ExtnMessage {
                    id: "some-uuid".to_string(),
                    requestor: ExtnId::get_main_target("main".into()),
                    target: RippleContract::Internal,
                    target_id: None,
                    payload: ExtnPayload::Response(exp_resp.clone().unwrap()),
                    callback: None,
                    ts: Some(Utc::now().timestamp_millis()),
                };

                assert!(Uuid::parse_str(&actual_response.id).is_ok());
                assert_eq!(actual_response.requestor, expected_message.requestor);
                assert_eq!(actual_response.target, expected_message.target);
                assert_eq!(actual_response.target_id, expected_message.target_id);

                assert_eq!(
                    actual_response.callback.is_some(),
                    expected_message.callback.is_some()
                );
                assert!(actual_response.ts.is_some());
            }
            Err(_) => {
                panic!("Received an unexpected error");
            }
        }
    }

    // TODO:
    // Fix the test case 1 - runs forever - even in extn_client event test it happens
    // update error messages in start_rx to validate
    // how to get the callback and validate - here & also in extn_client
    // add assertion for the callback
    // add assertion for the response in callback - throughout tests

    #[rstest(
        exp_resp,
        case(Some(ExtnResponse::Boolean(true))),
        case(Some(ExtnResponse::Boolean(false))),
        case(None)
    )]
    #[tokio::test]
    async fn test_event_processor_run(exp_resp: Option<ExtnResponse>) {
        let (mock_sender, mock_rx) = ExtnSender::mock();
        let mut extn_client = ExtnClient::new(mock_rx, mock_sender.clone());
        let processor = MockEventProcessor {
            state: MockState {
                client: extn_client.clone(),
                contracts: vec![RippleContract::Internal],
            },
            streamer: DefaultExtnStreamer::new(),
        };

        extn_client.add_event_processor(processor);
        let extn_client_for_thread = extn_client.clone();

        extn_client.clone().add_sender(
            ExtnId::get_main_target("main".into()),
            ExtnSymbol {
                id: "id".to_string(),
                uses: vec!["uses".to_string()],
                fulfills: Vec::new(),
                config: None,
            },
            mock_sender.tx,
        );

        tokio::spawn(async move {
            extn_client_for_thread.initialize().await;
        });

        let response = extn_client.event(MockEvent {
            event_name: "test_event".to_string(),
            result: serde_json::json!({"result": "result"}),
            context: None,
            app_id: Some("some_id".to_string()),
            expected_response: exp_resp,
        });

        match response {
            Ok(_) => {
                // nothing to assert here
            }
            Err(_) => {
                panic!("Received an unexpected error");
            }
        }

        // TODO - verify the event response in other sender?
    }
}
