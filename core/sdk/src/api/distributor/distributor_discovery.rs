// Copyright 2023 Comcast Cable Communications Management, LLC
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
//
// SPDX-License-Identifier: Apache-2.0
//

use serde::{Deserialize, Serialize};

use crate::{
    api::firebolt::fb_discovery::{
        ClearContentSetParams, ContentAccessListSetParams, MediaEventsAccountLinkRequestParams,
        SignInRequestParams,
    },
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

use super::distributor_request::DistributorRequest;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum DiscoveryRequest {
    SetContentAccess(ContentAccessListSetParams),
    ClearContent(ClearContentSetParams),
    SignIn(SignInRequestParams),
}

impl ExtnPayloadProvider for DiscoveryRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Distributor(DistributorRequest::Discovery(d))) =
            payload
        {
            return Some(d);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Distributor(DistributorRequest::Discovery(
            self.clone(),
        )))
    }

    fn contract() -> RippleContract {
        RippleContract::Discovery
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum MediaEventRequest {
    MediaEventAccountLink(MediaEventsAccountLinkRequestParams),
}

impl ExtnPayloadProvider for MediaEventRequest {
    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Distributor(DistributorRequest::MediaEvent(d))) =
            payload
        {
            return Some(d);
        }

        None
    }

    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Distributor(DistributorRequest::MediaEvent(
            self.clone(),
        )))
    }

    fn contract() -> RippleContract {
        RippleContract::MediaEvents
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::firebolt::fb_discovery::{
        ContentAccessEntitlement, ContentAccessInfo, ContentAccessListSetParams, SessionParams,
    };
    use crate::api::firebolt::fb_discovery::{MediaEvent, ProgressUnit};
    use crate::api::session::AccountSession;
    use crate::utils::test_utils::test_extn_payload_provider;
    use std::collections::HashSet;

    #[test]
    fn test_extn_request_discovery() {
        let content_access_entitlement = ContentAccessEntitlement {
            entitlement_id: "test_entitlement_id".to_string(),
            start_time: Some("2024-01-26T12:00:00Z".to_string()),
            end_time: Some("2024-02-01T12:00:00Z".to_string()),
        };

        let content_access_info = ContentAccessInfo {
            availabilities: Some(vec![]),
            entitlements: Some(vec![content_access_entitlement]),
        };

        let session_params = SessionParams {
            app_id: "test_app_id".to_string(),
            dist_session: AccountSession {
                id: "test_session_id".to_string(),
                token: "test_token".to_string(),
                account_id: "test_account_id".to_string(),
                device_id: "test_device_id".to_string(),
            },
        };

        let content_access_list_set_params = ContentAccessListSetParams {
            session_info: session_params,
            content_access_info,
        };

        let discovery_request = DiscoveryRequest::SetContentAccess(content_access_list_set_params);
        let contract_type: RippleContract = RippleContract::Discovery;

        test_extn_payload_provider(discovery_request, contract_type);
    }

    #[test]
    fn test_extn_payload_provider_for_media_event_request() {
        let media_event_request =
            MediaEventRequest::MediaEventAccountLink(MediaEventsAccountLinkRequestParams {
                media_event: MediaEvent {
                    content_id: String::from("your_content_id"),
                    completed: true,
                    progress: 0.75,
                    progress_unit: Some(ProgressUnit::Percent),
                    watched_on: Some(String::from("2024-01-26")),
                    app_id: String::from("your_app_id"),
                },
                content_partner_id: String::from("your_content_partner_id"),
                client_supports_opt_out: true,
                dist_session: AccountSession {
                    id: String::from("your_session_id"),
                    token: String::from("your_token"),
                    account_id: String::from("your_account_id"),
                    device_id: String::from("your_device_id"),
                },
                data_tags: HashSet::new(),
            });

        let contract_type: RippleContract = RippleContract::MediaEvents;
        test_extn_payload_provider(media_event_request, contract_type);
    }
}
