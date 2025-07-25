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

use std::sync::Arc;

use ripple_sdk::{
    api::{
        apps::{AppMethod, AppRequest, AppResponse},
        firebolt::fb_lifecycle_management::LifecycleManagementRequest,
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnPayload, ExtnPayloadProvider},
    },
    log::error,
    tokio::sync::{mpsc::Sender, oneshot},
};

use crate::service::extn::ripple_client::RippleClient;

/// Processor to service incoming Lifecycle Requests from launcher extension.
#[derive(Debug)]
pub struct LifecycleManagementProcessor {
    client: Arc<RippleClient>,
    streamer: DefaultExtnStreamer,
}

impl LifecycleManagementProcessor {
    pub fn new(client: Arc<RippleClient>) -> LifecycleManagementProcessor {
        LifecycleManagementProcessor {
            client: client.clone(),
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for LifecycleManagementProcessor {
    type STATE = Arc<RippleClient>;
    type VALUE = LifecycleManagementRequest;
    fn get_state(&self) -> Self::STATE {
        self.client.clone()
    }

    fn sender(&self) -> Sender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> ripple_sdk::tokio::sync::mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for LifecycleManagementProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.client.get_extn_client()
    }

    async fn process_request(state: Self::STATE, msg: ExtnMessage, request: Self::VALUE) -> bool {
        let (resp_tx, resp_rx) = oneshot::channel::<AppResponse>();
        let method = match request {
            LifecycleManagementRequest::Session(s) => AppMethod::BrowserSession(s.session),
            LifecycleManagementRequest::SetState(s) => AppMethod::SetState(s.app_id, s.state),
            LifecycleManagementRequest::Close(app_id, cr) => AppMethod::Close(app_id, cr),
            LifecycleManagementRequest::Ready(app_id) => AppMethod::Ready(app_id),
            LifecycleManagementRequest::GetSecondScreenPayload(app_id) => {
                AppMethod::GetSecondScreenPayload(app_id)
            }
            LifecycleManagementRequest::StartPage(app_id) => AppMethod::GetStartPage(app_id),
        };
        if let Err(e) = state.send_app_request(AppRequest::new(method, resp_tx)) {
            error!("Sending to App manager {:?}", e);
            return Self::handle_error(state.get_extn_client(), msg, e).await;
        }
        let resp = resp_rx.await;
        if let Ok(app_response) = resp {
            if let ExtnPayload::Response(payload) = app_response.get_extn_payload() {
                return Self::respond(state.get_extn_client(), msg, payload)
                    .await
                    .is_ok();
            }
        }

        true
    }
}
