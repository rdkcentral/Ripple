use crate::{
    firebolt::{
        firebolt_gateway::FireboltGateway, handlers::device_rpc::DeviceRPCProvider,
        rpc::RippleRPCProvider,
    },
    service::extn::ripple_client::RippleClient,
    state::bootstrap_state::BootstrapState,
};
use jsonrpsee::core::{async_trait, server::rpc_module::Methods};
use ripple_sdk::{framework::bootstrap::Bootstep, utils::error::RippleError};

pub struct FireboltGatewayStep;

impl FireboltGatewayStep {
    async fn init_handlers(
        &self,
        ripple_client: RippleClient,
        extn_methods: Option<Methods>,
    ) -> Methods {
        let mut methods = Methods::new();
        let _ = methods.merge(DeviceRPCProvider::provide(ripple_client.clone()));
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
        let ripple_client = state.platform_state.get_client();
        let methods = self.init_handlers(ripple_client, None).await;
        let gateway = FireboltGateway::new(state.clone(), methods);
        gateway.start().await;
        Ok(())
    }
}
