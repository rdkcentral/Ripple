use ripple_sdk::{
    api::gateway::rpc_gateway_api::RpcRequest,
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::ExtnMessage,
    },
    tokio::sync::mpsc::Sender,
};

use crate::{
    firebolt::firebolt_gateway::FireboltGatewayCommand, service::extn::ripple_client::RippleClient,
};

/// Processor to service incoming RPC Requests used by extensions and other local rpc handlers for aliasing.
#[derive(Debug)]
pub struct RpcGatewayProcessor {
    client: RippleClient,
    streamer: DefaultExtnStreamer,
}

impl RpcGatewayProcessor {
    pub fn new(client: RippleClient) -> RpcGatewayProcessor {
        RpcGatewayProcessor {
            client,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for RpcGatewayProcessor {
    type STATE = RippleClient;
    type VALUE = RpcRequest;
    fn get_state(&self) -> Self::STATE {
        self.client.clone()
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
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.client.get_extn_client()
    }

    async fn process_request(state: Self::STATE, msg: ExtnMessage, _request: Self::VALUE) -> bool {
        // Notice how this processor is different from others where it doesnt respond to
        // Self::respond this processor delegates the request down
        // to the gateway which does more complex inter connected operations. The design for
        // Extn Processor is built in such a way to support transient processors which do not
        // necessarily need to provide response
        if let Err(e) = state
            .send_gateway_command(FireboltGatewayCommand::HandleRpcForExtn { msg: msg.clone() })
        {
            return Self::handle_error(state.get_extn_client(), msg, e).await;
        }
        true
    }
}
