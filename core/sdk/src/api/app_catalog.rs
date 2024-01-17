use serde::{Deserialize, Serialize};

use crate::{
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AppCatalogRequest {
    OnAppsUpdate,
    CheckForUpdates,
}

impl ExtnPayloadProvider for AppCatalogRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::AppCatalog(v)) = payload {
            return Some(v);
        }
        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::AppCatalog(self.clone()))
    }

    fn contract() -> RippleContract {
        RippleContract::AppCatalog
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppMetadata {
    pub id: String,
    pub title: String,
    pub version: String,
    pub uri: String,
    pub data: Option<String>,
}

impl AppMetadata {
    pub fn new(
        id: String,
        title: String,
        version: String,
        uri: String,
        data: Option<String>,
    ) -> AppMetadata {
        AppMetadata {
            id,
            title,
            version,
            uri,
            data,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppsUpdate {
    pub apps: Vec<AppMetadata>,
}

impl AppsUpdate {
    pub fn new(apps: Vec<AppMetadata>) -> AppsUpdate {
        AppsUpdate { apps }
    }
}

impl ExtnPayloadProvider for AppsUpdate {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Event(ExtnEvent::AppsUpdate(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Event(ExtnEvent::AppsUpdate(apps_update)) = payload {
            return Some(apps_update);
        }

        None
    }

    fn contract() -> RippleContract {
        RippleContract::AppCatalog
    }
}
