use crate::thunder_state::ThunderBootstrapState;
use ripple_sdk::{
    api::config::Config,
    framework::bootstrap::Bootstep,
    log::{debug, warn},
    utils::error::RippleError,
};
use ripple_sdk::{
    async_trait::async_trait,
    extn::extn_client_message::{ExtnMessage, ExtnResponse},
};
use serde::Deserialize;
use serde_json::Error;
pub struct ThunderGetConfigStep;

const GATEWAY_DEFAULT: &'static str = "ws://127.0.0.1:9998/jsonrpc";
const POOL_SIZE_DEFAULT: u32 = 5;

#[derive(Deserialize, Clone)]
pub struct ThunderPlatformParameters {
    #[serde(default = "gateway_default")]
    gateway: String,
    #[serde(default = "pool_size_default")]
    pool_size: u32,
}

fn gateway_default() -> String {
    String::from(GATEWAY_DEFAULT)
}

fn pool_size_default() -> u32 {
    POOL_SIZE_DEFAULT
}

#[async_trait]
impl Bootstep<ThunderBootstrapState> for ThunderGetConfigStep {
    fn get_name(&self) -> String {
        "ThunderGetConfigStep".into()
    }

    async fn setup(&self, state: ThunderBootstrapState) -> Result<(), RippleError> {
        let extn_message_response: Result<ExtnMessage, RippleError> =
            state.get().send_payload(Config::PlatformParameters).await;
        if let Ok(message) = extn_message_response {
            if let Some(ExtnResponse::Value(v)) = message.payload.clone().extract() {
                let tp_res: Result<ThunderPlatformParameters, Error> = serde_json::from_value(v);
                let mut gateway_url = url::Url::parse(GATEWAY_DEFAULT).unwrap();
                if let Ok(thunder_parameters) = tp_res {
                    if let Ok(gurl) = url::Url::parse(&thunder_parameters.gateway) {
                        debug!("Got url from device manifest");
                        gateway_url = gurl
                    } else {
                        warn!(
                            "Could not parse thunder gateway '{}', using default {}",
                            thunder_parameters.gateway, GATEWAY_DEFAULT
                        );
                    }
                    state.pool_size(thunder_parameters.pool_size);
                } else {
                    warn!(
                        "Could not read thunder platform parameters, using default {}",
                        GATEWAY_DEFAULT
                    );
                }
                if let Ok(host_override) = std::env::var("THUNDER_HOST") {
                    gateway_url.set_host(Some(&host_override)).ok();
                }
                state.set_url(gateway_url);
                return Ok(());
            }
        }

        Err(RippleError::BootstrapError)
    }
}
