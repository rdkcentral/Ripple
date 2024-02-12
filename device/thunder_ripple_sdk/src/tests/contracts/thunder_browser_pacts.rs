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

use crate::get_pact_with_params;
use crate::processors::thunder_browser::ThunderBrowserRequestProcessor;
use crate::ripple_sdk::extn::client::extn_processor::ExtnRequestProcessor;
use crate::tests::contracts::contract_utils::*;
use crate::thunder_state::ThunderConnectionState;
use crate::{client::thunder_client_pool::ThunderClientPool, thunder_state::ThunderState};
use pact_consumer::mock_server::StartMockServerAsync;
use ripple_sdk::{
    api::device::{
        device_browser::{BrowserDestroyParams, BrowserLaunchParams, BrowserRequest},
        device_request::DeviceRequest,
    },
    async_channel::unbounded,
    extn::extn_client_message::{ExtnPayload, ExtnRequest},
    serde_json, tokio,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
// #[tokio::test(flavor = "multi_thread")]
// #[cfg_attr(not(feature = "contract_tests"), ignore)]
#[allow(dead_code)]
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
            "https://google.com".into(),
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
    let uri = "https://google.com";
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
        visible,
        suspend,
        focused,
        name: name.to_string(),
        x,
        y,
        w,
        h,
        properties: None,
    };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Browser(
        BrowserRequest::Start(start_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderBrowserRequestProcessor::process_request(
        state,
        msg,
        BrowserRequest::Start(start_params.clone()),
    )
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
        .synchronous_message_interaction(
            "A request to destroy an application",
            |mut i| async move {
                i.contents_from(get_pact_with_params!(
                    "org.rdk.RDKShell.1.destroy",
                    ContractResult { result },
                    ContractParams { params }
                ))
                .await;
                i.test_name("destroy_application");
                i
            },
        )
        .await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let browser_name = "Html-0";
    let destroy_params = BrowserDestroyParams {
        browser_name: browser_name.to_string(),
    };
    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Browser(
        BrowserRequest::Destroy(destroy_params.clone()),
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client =
        ThunderClientPool::start(url, None, Arc::new(ThunderConnectionState::new()), 1)
            .await
            .unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ = ThunderBrowserRequestProcessor::process_request(
        state,
        msg,
        BrowserRequest::Destroy(destroy_params.clone()),
    )
    .await;
}
