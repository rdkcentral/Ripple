use serde::{Deserialize, Serialize};

use crate::{
    extn::extn_client_message::{ExtnEvent, ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AppCatalogRequest {
    CheckForUpdates,
    GetCatalog,
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
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
pub struct AppsCatalogUpdate {
    pub old_catalog: Option<Vec<AppMetadata>>,
    pub new_catalog: Vec<AppMetadata>,
}

impl AppsCatalogUpdate {
    pub fn new(
        old_catalog: Option<Vec<AppMetadata>>,
        new_catalog: Vec<AppMetadata>,
    ) -> AppsCatalogUpdate {
        AppsCatalogUpdate {
            old_catalog,
            new_catalog,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AppsUpdate {
    InstallComplete(AppOperationComplete),
    UninstallComplete(AppOperationComplete),
    AppsCatalogUpdate(AppsCatalogUpdate),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AppOperationComplete {
    pub id: String,
    pub version: String,
    pub success: bool,
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
        RippleContract::Apps
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
        let apps_update = AppsUpdate::AppsCatalogUpdate(AppsCatalogUpdate {
            old_catalog: None,
            new_catalog: vec![AppMetadata {
                id: String::from("app1"),
                title: String::from("App 1"),
                version: String::from("1.0"),
                uri: String::from("https://example.com/app1"),
                data: Some(String::from("app1_data")),
                install_priority: 9999,
            }],
        });
        let contract_type: RippleContract = RippleContract::Apps;
        test_extn_payload_provider(apps_update, contract_type);
    }
}
