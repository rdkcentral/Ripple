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

use ripple_sdk::{
    api::firebolt::fb_secure_storage::{
        SecureStorageDefaultResponse, SecureStorageGetResponse, SecureStorageRequest,
        SecureStorageResponse,
    },
    async_trait::async_trait,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
        },
        extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnResponse},
    },
    log::error,
};

pub struct DistributorSecureStorageProcessor {
    client: ExtnClient,
    streamer: DefaultExtnStreamer,
}

impl DistributorSecureStorageProcessor {
    pub fn new(client: ExtnClient) -> DistributorSecureStorageProcessor {
        DistributorSecureStorageProcessor {
            client,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for DistributorSecureStorageProcessor {
    type STATE = ExtnClient;
    type VALUE = SecureStorageRequest;

    fn get_state(&self) -> Self::STATE {
        self.client.clone()
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
impl ExtnRequestProcessor for DistributorSecureStorageProcessor {
    fn get_client(&self) -> ExtnClient {
        self.client.clone()
    }
    async fn process_request(
        state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            SecureStorageRequest::Set(_req, _session) => {
                if let Err(e) = state
                    .clone()
                    .respond(
                        msg,
                        ripple_sdk::extn::extn_client_message::ExtnResponse::SecureStorage(
                            SecureStorageResponse::Set(SecureStorageDefaultResponse {}),
                        ),
                    )
                    .await
                {
                    error!("Error sending back response {:?}", e);
                    return false;
                }
                true
            }
            SecureStorageRequest::Get(_app_id, _req, _session) => {
                let resp = SecureStorageGetResponse {
                    value: Some(String::from("VGhpcyBub3QgYSByZWFsIHRva2VuLgo=")),
                };

                if let Err(e) = state
                    .clone()
                    .respond(
                        msg,
                        if let ExtnPayload::Response(r) =
                            SecureStorageResponse::Get(resp).get_extn_payload()
                        {
                            r
                        } else {
                            ExtnResponse::Error(
                                ripple_sdk::utils::error::RippleError::ProcessorError,
                            )
                        },
                    )
                    .await
                {
                    error!("Error sending back response {:?}", e);
                    return false;
                }
                true
            }

            SecureStorageRequest::Remove(_req, _session) => {
                if let Err(e) = state
                    .clone()
                    .respond(
                        msg,
                        ripple_sdk::extn::extn_client_message::ExtnResponse::SecureStorage(
                            SecureStorageResponse::Remove(SecureStorageDefaultResponse {}),
                        ),
                    )
                    .await
                {
                    error!("Error sending back response {:?}", e);
                    return false;
                }
                true
            }

            SecureStorageRequest::Clear(_req, _session) => {
                if let Err(e) = state
                    .clone()
                    .respond(
                        msg,
                        ripple_sdk::extn::extn_client_message::ExtnResponse::SecureStorage(
                            SecureStorageResponse::Clear(SecureStorageDefaultResponse {}),
                        ),
                    )
                    .await
                {
                    error!("Error sending back response {:?}", e);
                    return false;
                }
                true
            }
        }
    }
}
