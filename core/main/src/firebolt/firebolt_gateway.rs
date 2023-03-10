use jsonrpsee::core::server::rpc_module::Methods;
use ripple_sdk::{
    api::gateway::rpc_gateway_api::{ApiProtocol, RpcRequest},
    extn::extn_client_message::ExtnMessage,
    log::{error, info},
    tokio,
};

use crate::{
    firebolt::firebolt_gatekeeper::FireboltGatekeeper,
    state::{bootstrap_state::BootstrapState, session_state::Session},
};

use super::rpc_router::RpcRouter;
pub struct FireboltGateway {
    state: BootstrapState,
}

#[derive(Debug, Clone)]
pub enum FireboltGatewayCommand {
    RegisterSession {
        session_id: String,
        session: Session,
    },
    UnregisterSession {
        session_id: String,
    },
    HandleRpc {
        request: RpcRequest,
    },
    HandleRpcForExtn {
        msg: ExtnMessage,
    },
}

impl FireboltGateway {
    pub fn new(state: BootstrapState, methods: Methods) -> FireboltGateway {
        state.platform_state.router_state.update_methods(methods);
        FireboltGateway { state }
    }

    pub async fn start(&self) {
        info!("Starting Gateway Listener");
        let mut firebolt_gateway_rx = self
            .state
            .channels_state
            .get_gateway_receiver()
            .expect("Gateway receiver to be available");
        while let Some(cmd) = firebolt_gateway_rx.recv().await {
            use FireboltGatewayCommand::*;

            match cmd {
                RegisterSession {
                    session_id,
                    session,
                } => {
                    self.state
                        .platform_state
                        .session_state
                        .add_session(session_id, session);
                }
                UnregisterSession { session_id } => self
                    .state
                    .platform_state
                    .session_state
                    .clear_session(session_id),
                HandleRpc { request } => self.handle(request, None).await,
                HandleRpcForExtn { msg } => {
                    if let Some(request) = msg.payload.clone().extract() {
                        self.handle(request, Some(msg)).await
                    } else {
                        error!("Not a valid RPC Request {:?}", msg);
                    }
                }
            }
        }
    }

    pub async fn handle(&self, request: RpcRequest, extn_msg: Option<ExtnMessage>) {
        info!(
            "Received Firebolt request {} {} {}",
            request.ctx.request_id, request.method, request.params_json
        );
        // First check sender if no sender no need to process
        let session_id = request.clone().ctx.session_id;
        let callback_c = extn_msg.clone();
        match request.clone().ctx.protocol {
            ApiProtocol::Extn => {
                if callback_c.is_none() || callback_c.unwrap().callback.is_none() {
                    error!("No callback for request {:?} ", request);
                    return;
                }
            }
            _ => {
                if !self
                    .state
                    .platform_state
                    .session_state
                    .has_session(session_id.clone())
                {
                    error!("No sender for request {:?} ", request);
                    return;
                }
            }
        }
        let platform_state = self.state.platform_state.clone();
        let router_state = self.state.platform_state.router_state.clone();
        /*
         * The reason for spawning a new thread is that when request-1 comes, and it waits for
         * user grant. The response from user grant, (eg ChallengeResponse) comes as rpc which
         * in-turn goes through the permission check and sends a gate request. But the single
         * thread which was listening on the channel will be waiting for the user response. This
         * leads to a stall.
         */
        tokio::spawn(async move {
            match FireboltGatekeeper::gate(platform_state.clone(), request.clone()).await {
                Ok(_) => {
                    // Route
                    match request.clone().ctx.protocol {
                        ApiProtocol::Extn => {
                            RpcRouter::route_extn_protocol(
                                router_state.clone(),
                                request.clone(),
                                extn_msg.unwrap(),
                            )
                            .await
                        }
                        _ => {
                            let session =
                                platform_state.clone().session_state.get_sender(session_id);
                            // session is already precheched before gating so it is safe to unwrap
                            RpcRouter::route(
                                router_state.clone(),
                                request.clone(),
                                session.unwrap(),
                            )
                            .await;
                        }
                    }
                }
                Err(e) => {
                    // return error for Api message
                    error!("Failed gateway present error {:?}", e);
                }
            }
        });
    }
}
