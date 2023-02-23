use ripple_sdk::{
    api::gateway::rpc_gateway_api::RpcRequest,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer},
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
    streamer: ExtnStreamer,
}

impl RpcGatewayProcessor {
    pub fn new(state: ChannelsState) -> RpcGatewayProcessor {
        RpcGatewayProcessor {
            sender: state.get_gateway_sender(),
            streamer: ExtnStreamer::new(),
        }
    }
}

#[async_trait]
impl ExtnStreamProcessor for RpcGatewayProcessor {
    type S = Sender<FireboltGatewayCommand>;
    type V = RpcRequest;

    fn get_state(&self) -> Self::S {
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
        _state: Self::S,
        _msg: ExtnMessage,
        _error: ripple_sdk::utils::error::RippleError,
    ) -> Option<bool> {
        error!("parsing the request");
        None
    }

    async fn process_request(state: Self::S, msg: ExtnMessage, _request: Self::V) -> Option<bool> {
        if let Err(e) = state
            .send(FireboltGatewayCommand::HandleRpcForExtn { msg })
            .await
        {
            error!("error trying to send {:?}", e);
        }
        None
    }
}
