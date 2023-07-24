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

use serde_json::json;
use ripple_sdk::{
    api::device::{
        device_browser::{
            BrowserDestroyParams, BrowserLaunchParams, BrowserNameRequestParams, BrowserRequest, BrowserProps,
        },
        device_operator::{DeviceCallRequest, DeviceChannelParams, DeviceOperator},
        device_request::DeviceRequest,
    },
    async_trait::async_trait,
    extn::{
        client::{
            extn_client::ExtnClient,
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            },
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    crossbeam::channel::unbounded,
    extn::extn_client_message::{ExtnPayload, ExtnRequest},
    framework::RippleResponse,
    log::error,
    serde_json,
    tokio::sync::mpsc,
    utils::error::RippleError,
};
use serde::{Deserialize, Serialize};
use crate::{client::thunder_plugin::ThunderPlugin, thunder_state::ThunderState, client::thunder_client_pool::ThunderClientPool};
use crate::get_pact_with_params;
use crate::tests::contracts::contract_utils::*;
use pact_consumer::mock_server::StartMockServerAsync;
use std::collections::HashMap;

#[derive(Debug)]
pub struct ThunderBrowserRequestProcessor {
    state: ThunderState,
    streamer: DefaultExtnStreamer,
}

#[derive(Debug, Serialize, Deserialize)]
struct RDKShellLaunchRequest {
    callsign: String,
    #[serde(rename = "type")]
    _type: String,
    suspend: bool,
    uri: String,
    visible: bool,
    focused: bool,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct RDKShellDestroyRequest {
    callsign: String,
}

impl ThunderBrowserRequestProcessor {
    pub fn new(state: ThunderState) -> ThunderBrowserRequestProcessor {
        ThunderBrowserRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn handle_local_storage(
        state: ThunderState,
        browser_name: String,
        value: bool,
        req: ExtnMessage,
    ) -> RippleResponse {
        state
            .get_thunder_client()
            .call(DeviceCallRequest {
                method: format!("{}.localstorageenabled", browser_name),
                params: Some(DeviceChannelParams::Bool(value)),
            })
            .await;
        if let Err(_e) = Self::respond(state.get_client(), req, ExtnResponse::None(())).await {
            error!("Sending back response for localstorage ");
            return Err(RippleError::SendFailure);
        }

        Ok(())
    }

    async fn start(
        state: ThunderState,
        launch_params: BrowserLaunchParams,
        req: ExtnMessage,
    ) -> bool {
        let thunder_method = ThunderPlugin::RDKShell.method("launch");
        let client = state.get_thunder_client();
        let browser_name = launch_params.browser_name.clone();
        let lc_enabled = launch_params.is_local_storage_enabled();
        // check object integrity
        let r = RDKShellLaunchRequest {
            callsign: launch_params.browser_name,
            _type: launch_params._type,
            suspend: launch_params.suspend,
            uri: launch_params.uri,
            visible: launch_params.visible,
            focused: launch_params.focused,
            x: launch_params.x,
            y: launch_params.y,
            w: launch_params.w,
            h: launch_params.h,
        };
        let response = client
            .call(DeviceCallRequest {
                method: thunder_method,
                params: Some(DeviceChannelParams::Json(
                    serde_json::to_string(&r).unwrap(),
                )),
            })
            .await;
        if let Some(status) = response.message["success"].as_bool() {
            if status {
                return Self::handle_local_storage(
                    state.clone(),
                    browser_name,
                    lc_enabled,
                    req.clone(),
                )
                .await
                .is_ok();
            } else {
                return Self::respond(state.get_client(), req.clone(), ExtnResponse::None(()))
                    .await
                    .is_ok();
            }
        }
        Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
    }

    async fn destroy(
        state: ThunderState,
        destroy_params: BrowserDestroyParams,
        req: ExtnMessage,
    ) -> bool {
        let params = RDKShellDestroyRequest {
            callsign: destroy_params.browser_name,
        };
        let device_call_request = DeviceCallRequest {
            method: ThunderPlugin::RDKShell.method("destroy"),
            params: Some(DeviceChannelParams::Json(
                serde_json::to_string(&params).unwrap(),
            )),
        };
        let response = state.get_thunder_client().call(device_call_request).await;

        if let Some(status) = response.message["success"].as_bool() {
            if status {
                if let Err(_) =
                    Self::respond(state.get_client(), req.clone(), ExtnResponse::None(())).await
                {
                    error!("Sending back response for browser.destroy");
                }
                return false;
            }
        }
        Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await
    }

    async fn get_browser_name(
        state: ThunderState,
        bnr: BrowserNameRequestParams,
        req: ExtnMessage,
    ) -> bool {
        let browser_name = match bnr.runtime.as_str() {
            "web" => Some(format!("Html-{}", bnr.instances)),
            "lightning" => Some(format!("FireboltMainApp-{}", bnr.name)),
            _ => None,
        };
        if let None = browser_name {
            return Self::handle_error(state.get_client(), req, RippleError::ProcessorError).await;
        } else {
            return Self::respond(
                state.get_client(),
                req.clone(),
                ExtnResponse::String(browser_name.unwrap()),
            )
            .await
            .is_ok();
        }
    }
}

impl ExtnStreamProcessor for ThunderBrowserRequestProcessor {
    type STATE = ThunderState;
    type VALUE = BrowserRequest;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn receiver(&mut self) -> mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> mpsc::Sender<ExtnMessage> {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for ThunderBrowserRequestProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.get_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            BrowserRequest::Start(start_params) => {
                Self::start(state.clone(), start_params, msg).await
            }
            BrowserRequest::Destroy(destroy_params) => {
                Self::destroy(state.clone(), destroy_params, msg).await
            }
            BrowserRequest::GetBrowserName(browser_params) => {
                Self::get_browser_name(state.clone(), browser_params, msg).await
            }
        }
    }
}


#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_launch_html_app() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut params = HashMap::new();
    params.insert(
        "callsign".into(),
        ContractMatcher::MatchType("Html-0".into()),
    );
    params.insert("type".into(), ContractMatcher::MatchType("HtmlApp".into()));
    params.insert(
        "uri".into(),
        ContractMatcher::MatchRegex(
            r"^(([^:/?#]+):)?(//([^/?#]*))?([^?#]*)(\?([^#]*))?(#(.*))?".into(),
            "https://url_for_app".into(),
        ),
    );
    params.insert("x".into(), ContractMatcher::MatchNumber(0));
    params.insert("y".into(), ContractMatcher::MatchNumber(0));
    params.insert("w".into(), ContractMatcher::MatchNumber(1920));
    params.insert("h".into(), ContractMatcher::MatchNumber(1080));
    params.insert("suspend".into(), ContractMatcher::MatchBool(false));
    params.insert("visible".into(), ContractMatcher::MatchBool(true));
    params.insert("focused".into(), ContractMatcher::MatchBool(true));
    
    let mut result = HashMap::new();
    result.insert(
        "launchType".into(),
        ContractMatcher::MatchType("activate".into()),
    );
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction("A request to launch an application", |mut i| async move {
            i.contents_from(get_pact_with_params!(
                "org.rdk.RDKShell.1.launch",
                ContractResult { result },
                ContractParams { params }
            ))
            .await;
            i.test_name("lanuch_html_application");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let browser_name = "Html-0";
    let uri = "https://url_for_app";
    let _type = "HtmlApp";
    let visible = true;
    let suspend = false;
    let focused = true;
    let name = "Html-0";
    let x = 0;
    let y = 0;
    let w = 1920;
    let h = 1080;
    let start_params = BrowserLaunchParams {
        browser_name: browser_name.to_string(),
        uri: uri.to_string(),
        _type: _type.to_string(),
        visible: visible,
        suspend: suspend,
        focused: focused,
        name: name.to_string(),
        x: x,
        y: y,
        w: w,
        h: h,
        properties: None
    };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Browser(
        BrowserRequest::Start(start_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ =
    ThunderBrowserRequestProcessor::process_request(state, msg, BrowserRequest::Start(start_params.clone()))
            .await;
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_destroy_app() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    let mut params = HashMap::new();
    params.insert(
        "callsign".into(),
        ContractMatcher::MatchType("Html-0".into()),
    );
    
    let mut result = HashMap::new();
    result.insert("success".into(), ContractMatcher::MatchBool(true));

    pact_builder_async
        .synchronous_message_interaction("A request to destroy an application", |mut i| async move {
            i.contents_from(get_pact_with_params!(
                "org.rdk.RDKShell.1.destroy",
                ContractResult { result },
                ContractParams { params }
            ))
            .await;
            i.test_name("destroy_application");
            i
        })
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let browser_name = "Html-0";
    let destroy_params = BrowserDestroyParams {
        browser_name: browser_name.to_string()
    };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Browser(
        BrowserRequest::Destroy(destroy_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ =
    ThunderBrowserRequestProcessor::process_request(state, msg, BrowserRequest::Destroy(destroy_params.clone()))
            .await;
}