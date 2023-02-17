use crate::{
    firebolt::{
        firebolt_gateway::{FireboltGateway, FireboltGatewayCommand},
        handlers::device_rpc::DeviceRPCProvider,
        rpc::RippleRPCProvider,
    },
    service::extn::ripple_client::RippleClient,
    state::platform_state::PlatformState,
};
use jsonrpsee::core::{async_trait, server::rpc_module::Methods};
use ripple_sdk::{framework::bootstrap::Bootstep, tokio::sync::mpsc, utils::error::RippleError};

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
impl Bootstep<PlatformState> for FireboltGatewayStep {
    fn get_name(&self) -> String {
        "FireboltGatewayStep".into()
    }

    async fn setup(&self, state: PlatformState) -> Result<(), RippleError> {
        let ripple_client = state.client();
        let methods = self.init_handlers(ripple_client, None).await;
        let gateway = FireboltGateway::new(state.clone(), methods);
        let (firebolt_gateway_tx, firebolt_gateway_rx) =
            mpsc::channel::<FireboltGatewayCommand>(32);
        state.set_fb_gateway_sender(firebolt_gateway_tx);
        gateway.start(firebolt_gateway_rx).await;
        Ok(())
    }
}
