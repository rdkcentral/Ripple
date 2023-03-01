use crate::thunder_state::{ThunderBootstrapStateInitial, ThunderBootstrapStateWithConfig};
use ripple_sdk::extn::extn_client_message::{ExtnMessage, ExtnResponse};
use ripple_sdk::{
    api::config::Config,
    log::{debug, warn},
    utils::error::RippleError,
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

impl ThunderGetConfigStep {
    pub fn get_name() -> String {
        "ThunderGetConfigStep".into()
    }

    pub async fn setup(
        mut state: ThunderBootstrapStateInitial,
    ) -> Result<ThunderBootstrapStateWithConfig, RippleError> {
        let extn_message_response: Result<ExtnMessage, RippleError> =
            state.extn_client.request(Config::PlatformParameters).await;
        if let Ok(message) = extn_message_response {
            if let Some(ExtnResponse::Value(v)) = message.payload.clone().extract() {
                let mut pool_size = POOL_SIZE_DEFAULT;
                let tp_res: Result<ThunderPlatformParameters, Error> = serde_json::from_value(v);
                let mut gateway_url = url::Url::parse(GATEWAY_DEFAULT).unwrap();
                if let Ok(thunder_parameters) = tp_res {
                    pool_size = thunder_parameters.pool_size;
                    if let Ok(gurl) = url::Url::parse(&thunder_parameters.gateway) {
                        debug!("Got url from device manifest");
                        gateway_url = gurl
                    } else {
                        warn!(
                            "Could not parse thunder gateway '{}', using default {}",
                            thunder_parameters.gateway, GATEWAY_DEFAULT
                        );
                    }
                } else {
                    warn!(
                        "Could not read thunder platform parameters, using default {}",
                        GATEWAY_DEFAULT
                    );
                }
                if let Ok(host_override) = std::env::var("THUNDER_HOST") {
                    gateway_url.set_host(Some(&host_override)).ok();
                }
                return Ok(ThunderBootstrapStateWithConfig {
                    prev: state,
                    url: gateway_url,
                    pool_size,
                });
            }
        }

        Err(RippleError::BootstrapError)
    }
}
