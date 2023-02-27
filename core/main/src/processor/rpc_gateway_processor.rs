use ripple_sdk::{
    api::gateway::rpc_gateway_api::RpcRequest,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    log::error,
    tokio::sync::mpsc::Sender,
};

use crate::{
    firebolt::firebolt_gateway::FireboltGatewayCommand, state::bootstrap_state::ChannelsState,
};

/// Processor to service incoming RPC Requests used by extensions and other local rpc handlers for aliasing.
#[derive(Debug)]
pub struct RpcGatewayProcessor {
    sender: Sender<FireboltGatewayCommand>,
    streamer: DefaultExtnStreamer,
}

impl RpcGatewayProcessor {
    pub fn new(state: ChannelsState) -> RpcGatewayProcessor {
        RpcGatewayProcessor {
            sender: state.get_gateway_sender(),
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for RpcGatewayProcessor {
    type STATE = Sender<FireboltGatewayCommand>;
    type VALUE = RpcRequest;
    fn get_state(&self) -> Self::STATE {
        self.sender.clone()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> ripple_sdk::tokio::sync::mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for RpcGatewayProcessor {
    async fn process_error(
        _state: Self::STATE,
        _msg: ExtnMessage,
        _error: ripple_sdk::utils::error::RippleError,
    ) -> Option<bool> {
        error!("parsing the request");
        None
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        _request: Self::VALUE,
    ) -> Option<bool> {
        if let Err(e) = state
            .send(FireboltGatewayCommand::HandleRpcForExtn { msg })
            .await
        {
            error!("error trying to send {:?}", e);
        }
        None
    }
}
