// If not stated otherwise in this file or this component's license file the
// following copyright and licenses apply:
//
// Copyright 2023 RDK Management
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use serde::{Deserialize, Serialize};

use crate::{
    api::{firebolt::fb_capabilities::FireboltPermission, session::RippleSession},
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest, ExtnResponse},
    framework::ripple_contract::{DistributorContract, RippleContract},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub app_id: String,
    pub session: RippleSession,
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

pub type PermissionResponse = Vec<FireboltPermission>;

impl ExtnPayloadProvider for PermissionResponse {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        match payload {
            ExtnPayload::Response(r) => match r {
                ExtnResponse::Permission(v) => return Some(v),
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Response(ExtnResponse::Permission(self.clone()))
    }

    fn contract() -> crate::framework::ripple_contract::RippleContract {
        RippleContract::Distributor(DistributorContract::Permissions)
    }
}
