use serde::{Deserialize, Serialize};

use crate::{
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AppCatalogRequest {
    CheckForUpdates,
    // <pca>
    GetCatalog,
    // </pca>
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AppMetadata {
    pub id: String,
    pub title: String,
    pub version: String,
    pub uri: String,
    pub data: Option<String>,
    pub install_priority: i32,
}

impl AppMetadata {
    pub fn new(
        id: String,
        title: String,
        version: String,
        uri: String,
        data: Option<String>,
        install_priority: i32,
    ) -> AppMetadata {
        AppMetadata {
            id,
            title,
            version,
            uri,
            data,
            install_priority,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_extn_request_app_catalog() {
        let check_for_updates_request = AppCatalogRequest::CheckForUpdates;
        let contract_type: RippleContract = RippleContract::AppCatalog;
        test_extn_payload_provider(check_for_updates_request, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_apps_update() {
        let apps_update = AppsUpdate {
            apps: vec![AppMetadata {
                id: String::from("app1"),
                title: String::from("App 1"),
                version: String::from("1.0"),
                uri: String::from("https://example.com/app1"),
                data: Some(String::from("app1_data")),
                install_priority: 9999,
            }],
        };
        let contract_type: RippleContract = RippleContract::AppCatalog;
        test_extn_payload_provider(apps_update, contract_type);
    }
}
