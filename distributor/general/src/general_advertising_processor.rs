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
use ripple_sdk::{
    api::firebolt::fb_advertising::{AdIdResponse, AdInitObjectResponse, AdvertisingRequest},
    async_trait::async_trait,
    extn::client::{
        extn_client::ExtnClient,
        extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
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
        state: Self::STATE,
        msg: ripple_sdk::extn::extn_client_message::ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        println!("fasil 1");
        match extracted_message {
            AdvertisingRequest::ResetAdIdentifier(_session) => {
                println!("fasil 2");
                if let Err(e) = state
                    .clone()
                    .respond(
                        msg,
                        ripple_sdk::extn::extn_client_message::ExtnResponse::None(()),
                    )
                    .await
                {
                    error!("Error sending back response {:?}", e);
                    println!("fasil 3");
                    return false;
                }
                true
            }
            AdvertisingRequest::GetAdInitObject(_as_init_obj) => {
                if let Err(e) = state
                    .clone()
                    .respond(
                        msg,
                        ripple_sdk::extn::extn_client_message::ExtnResponse::AdInitObject(
                            AdInitObjectResponse{
                                    ad_server_url: "https://demo.v.fwmrm.net/ad/p/1".into(),
                                    ad_server_url_template: "https://demo.v.fwmrm.net/ad/p/1?flag=+sltp+exvt+slcb+emcr+amcb+aeti&prof=12345:caf_allinone_profile &nw=12345&mode=live&vdur=123&caid=a110523018&asnw=372464&csid=gmott_ios_tablet_watch_live_ESPNU&ssnw=372464&vip=198.205.92.1&resp=vmap1&metr=1031&pvrn=12345&vprn=12345&vcid=1X0Ce7L3xRWlTeNhc7br8Q%3D%3D".into(),
                                    ad_network_id: "519178".into(),
                                    ad_profile_id: "12345:caf_allinone_profile".into(),
                                    ad_site_section_id: "caf_allinone_profile_section".into(),
                                    ad_opt_out: true,
                                    privacy_data: "ew0KICAicGR0IjogImdkcDp2MSIsDQogICJ1c19wcml2YWN5IjogIjEtTi0iLA0KICAibG10IjogIjEiIA0KfQ0K".into(),
                                    ifa_value: "01234567-89AB-CDEF-GH01-23456789ABCD".into(),
                                    ifa: "ewogICJ2YWx1ZSI6ICIwMTIzNDU2Ny04OUFCLUNERUYtR0gwMS0yMzQ1Njc4OUFCQ0QiLAogICJpZmFfdHlwZSI6ICJzc3BpZCIsCiAgImxtdCI6ICIwIgp9Cg==".into(),
                                    app_name: "FutureToday".into(),
                                    app_version: "".into(),
                                    app_bundle_id: "FutureToday.comcast".into(),
                                    distributor_app_id: "1001".into(),
                                    device_ad_attributes: "ewogICJib0F0dHJpYnV0ZXNGb3JSZXZTaGFyZUlkIjogIjEyMzQiCn0=".into(),
                                    coppa: 0.to_string(),
                                    authentication_entity: "60f72475281cfba3852413bd53e957f6".into(),
                            },
                        ),
                    )
                    .await
                {
                    error!("Error sending back response {:?}", e);
                    return false;
                }
                true
            }

            AdvertisingRequest::GetAdIdObject(_ad_id_req) => {
                if let Err(e) = state
                    .clone()
                    .respond(
                        msg,
                        ripple_sdk::extn::extn_client_message::ExtnResponse::AdIdObject(
                            AdIdResponse {
                                ifa: "01234567-89AB-CDEF-GH01-23456789ABCD".into(),
                                ifa_type: "idfa".into(),
                                lmt: "0".into(),
                            },
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
