use serde_json::Value;
use tokio::sync::mpsc;

use crate::{
    async_channel::{unbounded, Sender},
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnRequest},
        extn_id::{ExtnClassId, ExtnId},
        ffi::ffi_message::CExtnMessage,
    },
    framework::ripple_contract::RippleContract,
    uuid::Uuid,
};

use super::extn_client_message::ExtnPayloadProvider;

#[derive(Debug)]
pub enum MockExtnRequest<T> {
    Message(ExtnMessage, T),
    Shutdown,
    ControlMessage(Value),
}

impl<T> MockExtnRequest<T> {
    pub fn as_msg(self) -> Option<(ExtnMessage, T)> {
        match self {
            MockExtnRequest::Message(m, em) => Some((m, em)),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct MockProcessorState<T> {
    pub sender: mpsc::Sender<MockExtnRequest<T>>,
}

#[macro_export]
macro_rules! create_processor {
    ($proc_ty:ident, $val_ty:ident) => {
        struct $proc_ty {
            client: $crate::extn::client::extn_client::ExtnClient,
            state: $crate::extn::mock_extension_client::MockProcessorState<$val_ty>,
            streamer: $crate::extn::client::extn_processor::DefaultExtnStreamer,
            contract: $crate::framework::ripple_contract::RippleContract,
        }

        impl $crate::extn::client::extn_processor::ExtnStreamProcessor for $proc_ty {
            type STATE = $crate::extn::mock_extension_client::MockProcessorState<$val_ty>;
            type VALUE = $val_ty;

            fn get_state(&self) -> Self::STATE {
                self.state.clone()
            }

            fn receiver(&mut self) -> $crate::tokio::sync::mpsc::Receiver<ExtnMessage> {
                self.streamer.receiver()
            }

            fn sender(&self) -> $crate::tokio::sync::mpsc::Sender<ExtnMessage> {
                self.streamer.sender()
            }

            fn contract(&self) -> $crate::framework::ripple_contract::RippleContract {
                self.contract.clone()
            }
        }

        #[$crate::async_trait::async_trait]
        impl $crate::extn::client::extn_processor::ExtnRequestProcessor for $proc_ty {
            fn get_client(&self) -> $crate::extn::client::extn_client::ExtnClient {
                self.client.clone()
            }
            async fn process_request(
                state: Self::STATE,
                msg: $crate::extn::extn_client_message::ExtnMessage,
                extracted_message: Self::VALUE,
            ) -> bool {
                state
                    .sender
                    .send(
                        $crate::extn::mock_extension_client::MockExtnRequest::Message(
                            msg,
                            extracted_message,
                        ),
                    )
                    .await
                    .is_ok()
            }
        }

        impl $proc_ty {
            fn add(
                client: &mut $crate::extn::client::extn_client::ExtnClient,
            ) -> (
                $crate::extn::mock_extension_client::MockProcessorClient<$val_ty>,
                $crate::tokio::sync::mpsc::Receiver<
                    $crate::extn::mock_extension_client::MockExtnRequest<$val_ty>,
                >,
            ) {
                let (extn_tx, extn_rx) = $crate::tokio::sync::mpsc::channel(1);

                let contract =
                    <$val_ty as $crate::extn::extn_client_message::ExtnPayloadProvider>::contract();
                let processor = $proc_ty {
                    client: client.clone(),
                    state: $crate::extn::mock_extension_client::MockProcessorState {
                        sender: extn_tx.clone(),
                    },
                    streamer: $crate::extn::client::extn_processor::DefaultExtnStreamer::new(),
                    contract: contract.clone(),
                };

                client.add_request_processor(processor);
                (
                    $crate::extn::mock_extension_client::MockProcessorClient { extn_tx, contract },
                    extn_rx,
                )
            }

            ///
            /// Creates an ExtnClient for main
            /// Creates and adds this processor to it
            /// Starts up ExtnClient
            /// Returns the ExtnClient, a ProcessorClient to communicate to the processor and the
            /// receiver for the processor to get messages
            fn mock_extn_client() -> (
                $crate::extn::client::extn_client::ExtnClient,
                $crate::extn::mock_extension_client::MockProcessorClient<$val_ty>,
                $crate::tokio::sync::mpsc::Receiver<
                    $crate::extn::mock_extension_client::MockExtnRequest<$val_ty>,
                >,
            ) {
                let mut main = $crate::extn::mock_extension_client::MockExtnClientUtil::main();
                let (cli, extn_rx) = $proc_ty::add(&mut main);
                $crate::extn::mock_extension_client::MockExtnClientUtil::start(main.clone());
                (main, cli, extn_rx)
            }
        }
    };
}

pub struct MockProcessorClient<T> {
    pub extn_tx: mpsc::Sender<MockExtnRequest<T>>,
    pub contract: RippleContract,
}

impl<T> MockProcessorClient<T> {
    pub async fn shutdown(&self) {
        self.extn_tx.send(MockExtnRequest::Shutdown).await.ok();
    }

    pub async fn send_control(&self, val: Value) {
        self.extn_tx
            .send(MockExtnRequest::ControlMessage(val))
            .await
            .ok();
    }
}

pub struct MockExtnClientUtil {}

impl MockExtnClientUtil {
    pub fn client(response_sender: Sender<CExtnMessage>) -> ExtnClient {
        let (_, ignore) = unbounded();
        ExtnClient::new(
            ignore,
            ExtnSender::new(
                response_sender,
                ExtnId::new_channel(ExtnClassId::Internal, "test".into()),
                Vec::new(),
                Vec::new(),
                None,
            ),
        )
    }

    pub fn main() -> ExtnClient {
        let (main_s, main_r) = async_channel::unbounded();
        ExtnClient::new(
            main_r,
            ExtnSender::new(
                main_s,
                ExtnId::get_main_target(String::from("main")),
                Vec::new(),
                Vec::new(),
                None,
            ),
        )
    }

    pub fn start(client: ExtnClient) {
        tokio::spawn(async move {
            client.initialize().await;
        });
    }

    pub fn req(contract: RippleContract, req: ExtnRequest) -> ExtnMessage {
        ExtnMessage {
            callback: None,
            id: Uuid::new_v4().to_string(),
            payload: ExtnPayload::Request(req),
            requestor: ExtnId::new_channel(ExtnClassId::Internal, "test".into()),
            target: contract,
            target_id: None,
            ts: Some(30),
        }
    }

    pub async fn respond_with_payload(
        client: &mut ExtnClient,
        req: ExtnMessage,
        resp: impl ExtnPayloadProvider,
    ) {
        if let ExtnPayload::Response(r) = resp.get_extn_payload() {
            client
                .send_message(req.get_response(r).unwrap())
                .await
                .expect("Could not send response")
        }
    }
}
