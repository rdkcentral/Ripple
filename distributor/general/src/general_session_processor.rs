use ripple_sdk::{
    api::distributor::distributor_session::{DistributorSession, DistributorSessionRequest},
    async_trait::async_trait,
    extn::client::{
        extn_client::ExtnClient,
        extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
    },
    log::error,
};

pub struct DistributorSessionProcessor {
    client: ExtnClient,
    streamer: DefaultExtnStreamer,
}

impl DistributorSessionProcessor {
    pub fn new(client: ExtnClient) -> DistributorSessionProcessor {
        DistributorSessionProcessor {
            client,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for DistributorSessionProcessor {
    type STATE = ExtnClient;
    type VALUE = DistributorSessionRequest;

    fn get_state(&self) -> Self::STATE {
        self.client.clone()
    }

    fn receiver(
        &mut self,
    ) -> ripple_sdk::tokio::sync::mpsc::Receiver<ripple_sdk::extn::extn_client_message::ExtnMessage>
    {
        self.streamer.receiver()
    }

    fn sender(
        &self,
    ) -> ripple_sdk::tokio::sync::mpsc::Sender<ripple_sdk::extn::extn_client_message::ExtnMessage>
    {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for DistributorSessionProcessor {
    fn get_client(&self) -> ExtnClient {
        self.client.clone()
    }
    async fn process_request(
        state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        _extracted_message: Self::VALUE,
    ) -> bool {
        if let Err(e) = state
            .clone()
            .respond(
                msg,
                ripple_sdk::extn::extn_client_message::ExtnResponse::Session(DistributorSession {
                    id: "general".into(),
                    token: "general".into(),
                    account_id: "general".into(),
                    device_id: "general".into(),
                }),
            )
            .await
        {
            error!("Error sending back response {:?}", e);
            return false;
        }
        true
    }
}
