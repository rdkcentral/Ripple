use crate::{
    firebolt::{
        firebolt_gateway::FireboltGateway,
        handlers::{device_rpc::DeviceRPCProvider, lcm_rpc::LifecycleManagementProvider},
        rpc::RippleRPCProvider,
    },
    processor::rpc_gateway_processor::RpcGatewayProcessor,
    state::{bootstrap_state::BootstrapState, platform_state::PlatformState},
};
use jsonrpsee::core::{async_trait, server::rpc_module::Methods};
use ripple_sdk::{framework::bootstrap::Bootstep, utils::error::RippleError};

pub struct FireboltGatewayStep;

impl FireboltGatewayStep {
    async fn init_handlers(&self, state: PlatformState, extn_methods: Option<Methods>) -> Methods {
        let mut methods = Methods::new();
        let _ = methods.merge(DeviceRPCProvider::provide(state.clone()));
        let _ = methods.merge(LifecycleManagementProvider::provide(state.clone()));
        if extn_methods.is_some() {
            let _ = methods.merge(extn_methods.unwrap());
        }
        methods
    }
}

#[async_trait]
impl Bootstep<BootstrapState> for FireboltGatewayStep {
    fn get_name(&self) -> String {
        "FireboltGatewayStep".into()
    }

    async fn setup(&self, state: BootstrapState) -> Result<(), RippleError> {
        let methods = self.init_handlers(state.platform_state.clone(), None).await;
        let gateway = FireboltGateway::new(state.clone(), methods);
        // Main can now recieve RPC requests
        state
            .platform_state
            .get_client()
            .add_request_processor(RpcGatewayProcessor::new(state.channels_state));
        gateway.start().await;
        Ok(())
    }
}
