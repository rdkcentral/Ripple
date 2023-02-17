use std::sync::{Arc, RwLock};

use ripple_sdk::{
    log::error,
    tokio::sync::mpsc::Sender,
    utils::error::RippleError,
};

use crate::firebolt::firebolt_gateway::FireboltGatewayCommand;

/// RippleClient is an internal delegate component which helps in operating
/// 1. ExtnClient 
/// 2. Firebolt Gateway
/// 3. Capabilities 
/// 4. Usergrants
///
/// # Examples
/// ```
/// use crate::firebolt::firebolt_gateway::FireboltGatewayCommand;
/// fn send_gateway_command(msg: FireboltGatewayCommand) {
///     let client = RippleClient::new();
///     client.send_gateway_command()
/// }
/// 
/// ```
#[derive(Debug, Clone)]
pub struct RippleClient {
    gateway_sender: Arc<RwLock<Option<Sender<FireboltGatewayCommand>>>>,
}

impl RippleClient {
    pub fn new() -> RippleClient {
        RippleClient {
            gateway_sender: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_gateway_sender(&self, sender: Sender<FireboltGatewayCommand>) {
        let mut sender_state = self.gateway_sender.write().unwrap();
        let _ = sender_state.insert(sender);
    }

    pub fn send_gateway_command(&self, cmd: FireboltGatewayCommand) -> Result<(), RippleError> {
        let sender_state = self.gateway_sender.read().unwrap();
        if sender_state.is_none() {
            return Err(RippleError::SenderMissing);
        }
        let sender = sender_state.clone().unwrap();
        if let Err(e) = sender.try_send(cmd) {
            error!("failed to send firebolt gateway message {:?}", e);
            return Err(RippleError::SendFailure);
        }
        Ok(())
    }
}
