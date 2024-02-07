use ripple_sdk::{
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
