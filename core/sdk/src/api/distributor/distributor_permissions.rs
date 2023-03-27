use serde::{Deserialize, Serialize};

use crate::{
    api::firebolt::fb_capabilities::FireboltPermission,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::{DistributorContract, RippleContract},
};

use super::distributor_session::DistributorSession;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub app_id: String,
    pub session: DistributorSession,
}

impl ExtnPayloadProvider for PermissionRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Request(r) => match r {
                ExtnRequest::Permission(p) => return Some(p),
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Permission(self.clone()))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::Distributor(DistributorContract::Permissions)
    }
}

pub type AppPermissionsResponse = Vec<FireboltPermission>;

impl ExtnPayloadProvider for AppPermissionsResponse {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Response(r) => match r {
                ExtnResponse::Value(v) => {
                    if let Ok(r) = serde_json::from_value(v) {
                        return Some(r);
                    }
                }
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Value(
            serde_json::to_value(self.clone()).unwrap(),
        ))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::Distributor(DistributorContract::Permissions)
    }
}
