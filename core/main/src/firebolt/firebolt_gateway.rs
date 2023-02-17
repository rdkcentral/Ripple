use jsonrpsee::core::server::rpc_module::Methods;
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{ApiMessage, RpcRequest},
    log::{error, info},
    tokio::sync::mpsc,
};

use crate::{firebolt::firebolt_permittor::FireboltGatewayPermitter, state::platform_state::PlatformState};

use super::rpc_router::RpcRouter;
pub struct FireboltGateway {
    state: PlatformState,
    router: RpcRouter,
}

#[derive(Debug)]
pub enum FireboltGatewayCommand {
    RegisterSession {
        session_id: String,
        session_tx: mpsc::Sender<ApiMessage>,
    },
    UnregisterSession {
        session_id: String,
    },
    HandleRpc {
        request: RpcRequest,
    }
}

impl FireboltGateway {
    pub fn new(state: PlatformState, methods: Methods) -> FireboltGateway {
        let router = RpcRouter::new(methods);
        FireboltGateway { router, state }
    }

    pub async fn start(&self, mut firebolt_gateway_rx: mpsc::Receiver<FireboltGatewayCommand>) {
        info!("Starting Gateway Listener");
        while let Some(cmd) = firebolt_gateway_rx.recv().await {
            use FireboltGatewayCommand::*;

            match cmd {
                RegisterSession {
                    session_id,
                    session_tx,
                } => {
                    self.state.session_state.add_session(session_id, session_tx);
                }
                UnregisterSession { session_id } => {
                    self.state.session_state.clear_session(session_id)
                    // TODO add provider broker
                    //ProviderBroker::unregister_session(&platform_state, session_id.clone()).await;
                }
                HandleRpc { request } => self.handle(request).await
            }
        }
    }

    pub async fn handle(&self, request: RpcRequest) {
        info!(
            "Received Firebolt request {} {} {}",
            request.ctx.request_id, request.method, request.params_json
        );
        // First check sender if no sender no need to process
        let session_id = request.clone().ctx.session_id;
        if !self.state.session_state.has_sender(session_id.clone()) {
            error!("No sender for request {:?} ", request);
            return;
        }
        match FireboltGatewayPermitter::gate(self.state.cap_state.clone(), request.clone()).await {
            Ok(_) => {
                // Route
                match request.clone().ctx.protocol {
                    _ => {
                        let sender = self.state.session_state.get_sender(session_id);
                        self.router.route(request.clone(), sender.unwrap()).await;
                    }
                }
            }
            Err(e) => {
                // return error for Api message
                error!("Failed gateway present error {:?}", e);
            }
        }
    }
}
