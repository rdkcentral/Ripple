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

use std::{collections::HashMap, fs, path::Path};

use ripple_sdk::{
    api::{
        distributor::distributor_permissions::PermissionRequest,
        firebolt::fb_capabilities::FireboltPermission,
    },
    async_trait::async_trait,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
        },
        extn_client_message::ExtnResponse,
    },
    log::{debug, error},
};

#[derive(Debug, Clone)]
pub struct PermissionState {
    client: ExtnClient,
    permissions: HashMap<String, Vec<FireboltPermission>>,
}

pub struct DistributorPermissionProcessor {
    state: PermissionState,
    streamer: DefaultExtnStreamer,
}

fn get_permissions_map() -> HashMap<String, Vec<FireboltPermission>> {
    if let Some(p) = Path::new("/opt/ripple/permissions_map.json").to_str() {
        if let Ok(contents) = fs::read_to_string(p) {
            if let Ok(r) = serde_json::from_str(contents.as_str()) {
                return r;
            }
        }
    }

    let contents = std::include_str!("./general_permissions_map.json");
    serde_json::from_str(contents).expect("valid permissions map")
}

impl DistributorPermissionProcessor {
    pub fn new(client: ExtnClient) -> DistributorPermissionProcessor {
        DistributorPermissionProcessor {
            state: PermissionState {
                client,
                permissions: get_permissions_map(),
            },
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for DistributorPermissionProcessor {
    type STATE = PermissionState;
    type VALUE = PermissionRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn receiver(
        &mut self,
    ) -> ripple_sdk::tokio::sync::mpsc::Receiver<ripple_sdk::extn::extn_client_message::ExtnMessage>
    {
        self.streamer.receiver()
    }

    fn sender(
        &self,
    ) -> ripple_sdk::tokio::sync::mpsc::Sender<ripple_sdk::extn::extn_client_message::ExtnMessage>
    {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for DistributorPermissionProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.client.clone()
    }
    async fn process_request(
        mut state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        if let Some(v) = state.permissions.get(&extracted_message.app_id) {
            debug!(
                "Permissions for app: {} [{:?}]",
                extracted_message.app_id, v
            );
            if let Err(e) = state
                .client
                .respond(msg, ExtnResponse::Permission(v.clone()))
                .await
            {
                error!("Error sending back response {:?}", e);
                return false;
            }
        } else {
            return Self::handle_error(
                state.client.clone(),
                msg,
                ripple_sdk::utils::error::RippleError::MissingInput,
            )
            .await;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use ripple_sdk::api::firebolt::fb_capabilities::FireboltCap;

    use super::get_permissions_map;

    // For sanity of the permissions map file
    #[test]
    fn test_permissions_map() {
        let v = get_permissions_map();
        assert!(!v.is_empty());
        assert!(!v.get("refui").unwrap().is_empty());
        let permission = v.get("refui").unwrap().get(1).unwrap().clone();
        println!("permission {}", permission.cap.as_str());
        assert!(FireboltCap::short("input:keyboard").eq(&permission.cap))
    }
}
