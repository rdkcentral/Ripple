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
    api::firebolt::fb_advertising::{
        AdConfigResponse, AdIdResponse, AdvertisingRequest, AdvertisingResponse,
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

pub struct DistributorAdvertisingProcessor {
    client: ExtnClient,
    streamer: DefaultExtnStreamer,
}

impl DistributorAdvertisingProcessor {
    pub fn new(client: ExtnClient) -> DistributorAdvertisingProcessor {
        DistributorAdvertisingProcessor {
            client,
            streamer: DefaultExtnStreamer::new(),
        }
    }
}

impl ExtnStreamProcessor for DistributorAdvertisingProcessor {
    type STATE = ExtnClient;
    type VALUE = AdvertisingRequest;

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
impl ExtnRequestProcessor for DistributorAdvertisingProcessor {
    fn get_client(&self) -> ExtnClient {
        self.client.clone()
    }
    async fn process_request(
        mut state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            AdvertisingRequest::ResetAdIdentifier(_session) => {
                if let Err(e) = state
                    .respond(
                        msg,
                        ripple_sdk::extn::extn_client_message::ExtnResponse::None(()),
                    )
                    .await
                {
                    error!("Error sending back response {:?}", e);
                    return false;
                }
                true
            }
            AdvertisingRequest::GetAdIdObject(_ad_id_req) => {
                let resp = AdIdResponse {
                    ifa: "01234567-89AB-CDEF-GH01-23456789ABCD".into(),
                    ifa_type: "idfa".into(),
                    lmt: "0".into(),
                };
                if let Err(e) = state
                    .respond(
                        msg,
                        if let ExtnPayload::Response(r) =
                            AdvertisingResponse::AdIdObject(resp).get_extn_payload()
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
            AdvertisingRequest::GetAdConfig(_config_req) => {
                let resp = AdConfigResponse {
                    ad_server_url: "https://some.host/ad/p/1".into(),
                    ad_server_url_template: "https://some.host/ad/p/1?flag=+sltp+exvt+slcb+emcr+amcb+aeti&prof=12345:caf_allinone_profile &nw=12345&mode=live&vdur=123&caid=a110523018&asnw=372464&csid=gmott_ios_tablet_watch_live_ESPNU&ssnw=372464&vip=198.205.92.1&resp=vmap1&metr=1031&pvrn=12345&vprn=12345&vcid=1X0Ce7L3xRWlTeNhc7br8Q%3D%3D".into(),
                    ad_network_id: "519178".into(),
                    ad_profile_id: "12345:caf_allinone_profile".into(),
                    ad_site_section_id: "caf_allinone_profile_section".into(),
                    ifa_value: "01234567-89AB-AHSG-GH01-23456789ABCD".into(),
                    ifa: "ewogICJ2YWx1ZSI6ICIwMTIzNDUKDSFHJDFKHJFKDSHFKJDHFhfQiLAogICJpZmFfdHlwZSI6ICJzc3BpZCIsCiAgImxtdCI6ICIwIgp9Cg==".into(),
                    app_bundle_id: "SomeApp.company".into(),
                };

                if let Err(e) = state
                    .respond(
                        msg,
                        if let ExtnPayload::Response(r) =
                            AdvertisingResponse::AdConfig(resp).get_extn_payload()
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
        }
    }
}
