use std::sync::{Arc, RwLock};

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
pub struct ThunderBootstrapState {
    url: Arc<RwLock<Option<Url>>>,
    pool_size: Arc<RwLock<Option<u32>>>,
    extns: Vec<DeviceExtn>,
    thunder_state: ThunderState,
}

impl ThunderBootstrapState {
    pub fn new(extn_client: ExtnClient, extns: Vec<DeviceExtn>) -> ThunderBootstrapState {
        ThunderBootstrapState {
            url: Arc::new(RwLock::new(None)),
            pool_size: Arc::new(RwLock::new(None)),
            extns,
            thunder_state: ThunderState::new(extn_client),
        }
    }

    pub fn get_url(&self) -> Url {
        let url = self.url.read().unwrap();
        url.clone().unwrap()
    }

    pub fn set_url(&self, url: Url) {
        let mut url_state = self.url.write().unwrap();
        let _ = url_state.insert(url);
    }

    pub fn pool_size(&self, size: u32) {
        let mut pool_size_state = self.pool_size.write().unwrap();
        let _ = pool_size_state.insert(size);
    }

    pub fn get_pool_size(&self) -> u32 {
        self.pool_size.read().unwrap().unwrap()
    }

    pub fn get_extns(&self) -> Vec<DeviceExtn> {
        self.extns.clone()
    }

    pub fn get(&self) -> ThunderState {
        self.thunder_state.clone()
    }
}

#[derive(Debug, Clone)]
pub struct ThunderState {
    extn_client: ExtnClient,
    thunder_client: Arc<RwLock<Option<ThunderClient>>>,
}

impl ThunderState {
    pub fn new(extn_client: ExtnClient) -> ThunderState {
        ThunderState {
            extn_client,
            thunder_client: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_thunder_client(&self, thunder_client: ThunderClient) {
        let mut thunder_pool_state = self.thunder_client.write().unwrap();
        let _ = thunder_pool_state.insert(thunder_client);
    }

    pub fn get_thunder_client(&self) -> ThunderClient {
        let thunder_client = self.thunder_client.read().unwrap();
        thunder_client.clone().unwrap()
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
