use ripple_sdk::{
    extn::{
        client::extn_client::ExtnClient,
        extn_client_message::{ExtnMessage, ExtnPayloadProvider},
        ffi::ffi_device::DeviceExtn,
    },
    utils::error::RippleError,
};
use url::Url;

use crate::client::thunder_client::ThunderClient;

#[derive(Debug, Clone)]
pub struct ThunderBootstrapStateInitial {
    pub extns: Vec<DeviceExtn>,
    pub extn_client: ExtnClient,
}

#[derive(Debug, Clone)]
pub struct ThunderBootstrapStateWithConfig {
    pub prev: ThunderBootstrapStateInitial,
    pub url: Url,
    pub pool_size: u32,
}

#[derive(Debug, Clone)]
pub struct ThunderBootstrapStateWithClient {
    pub prev: ThunderBootstrapStateWithConfig,
    pub state: ThunderState,
}

#[derive(Debug, Clone)]
pub struct ThunderState {
    extn_client: ExtnClient,
    thunder_client: ThunderClient,
}

impl ThunderState {
    pub fn new(extn_client: ExtnClient, thunder_client: ThunderClient) -> ThunderState {
        ThunderState {
            extn_client,
            thunder_client,
        }
    }

    pub fn get_thunder_client(&self) -> ThunderClient {
        self.thunder_client.clone()
    }

    pub fn get_client(&self) -> ExtnClient {
        self.extn_client.clone()
    }

    pub async fn send_payload(
        &self,
        payload: impl ExtnPayloadProvider,
    ) -> Result<ExtnMessage, RippleError> {
        self.extn_client.clone().request(payload).await
    }
}
