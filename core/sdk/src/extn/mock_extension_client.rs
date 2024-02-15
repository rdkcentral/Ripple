use crate::{
    api::manifest::extn_manifest::ExtnSymbol,
    async_channel::{unbounded, Receiver, Sender},
    extn::{
        client::{extn_client::ExtnClient, extn_sender::ExtnSender},
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnRequest},
        extn_id::{ExtnClassId, ExtnId},
        ffi::ffi_message::CExtnMessage,
    },
    framework::ripple_contract::RippleContract,
    tokio::spawn,
    uuid::Uuid,
};

#[derive(Debug, Clone)]
pub struct MockExtnClient {}

impl MockExtnClient {
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

    ///
    /// Creates an ExtnClient that has main able to communicate with an
    /// Extension. Returns a receiver that can be used to handle the extn
    /// messages to mock the responses from the extn
    /// This will start a thread so that the client can be initialized and listen for messages
    pub fn main_and_extn(extn_fulfills: Vec<String>) -> (ExtnClient, Receiver<CExtnMessage>) {
        let (main_s, main_r) = unbounded();
        let (extn_s, extn_r) = unbounded();
        let mut client = ExtnClient::new(
            main_r,
            ExtnSender::new(
                main_s,
                ExtnId::get_main_target(String::from("main")),
                Vec::new(),
                Vec::new(),
                None,
            ),
        );
        let symbol = ExtnSymbol {
            id: String::from("test"),
            uses: vec![],
            fulfills: extn_fulfills,
            config: None,
        };
        client.add_sender(
            ExtnId::new_channel(ExtnClassId::Internal, "test".into()),
            symbol,
            extn_s,
        );
        let main_client = client.clone();
        spawn(async move {
            main_client.initialize().await;
        });
        (client, extn_r)
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
}
