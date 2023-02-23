use ripple_sdk::{log::error, tokio::sync::mpsc::Sender, utils::error::RippleError};

use crate::{
    firebolt::firebolt_gateway::FireboltGatewayCommand, state::bootstrap_state::ChannelsState,
};

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
    gateway_sender: Sender<FireboltGatewayCommand>,
}

impl RippleClient {
    pub fn new(state: ChannelsState) -> RippleClient {
        RippleClient {
            gateway_sender: state.get_gateway_sender(),
        }
    }

    pub fn send_gateway_command(&self, cmd: FireboltGatewayCommand) -> Result<(), RippleError> {
        if let Err(e) = self.gateway_sender.try_send(cmd) {
            error!("failed to send firebolt gateway message {:?}", e);
            return Err(RippleError::SendFailure);
        }
        Ok(())
    }
}
