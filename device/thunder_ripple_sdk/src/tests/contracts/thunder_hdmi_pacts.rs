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

use crate::{
    client::thunder_client_pool::ThunderClientPool,
    ripple_sdk::{
        crossbeam::channel::unbounded,
        extn::extn_client_message::{ExtnPayload, ExtnRequest},
    },
    thunder_state::ThunderState,
};

use crate::ripple_sdk::{
    api::device::{
        device_request::DeviceRequest,
        panel::device_hdmi::HdmiRequest,
    },
    extn::client::extn_processor::ExtnRequestProcessor,
    serde_json::{self},
    tokio,
};

use crate::processors::panel::thunder_hdmi::ThunderHdmiRequestProcessor;

use crate::tests::contracts::contract_utils::*;
use pact_consumer::mock_server::StartMockServerAsync;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(not(feature = "contract_tests"), ignore)]
async fn test_device_hdmi_ports() {
    let mut pact_builder_async = get_pact_builder_async_obj().await;

    pact_builder_async
    .synchronous_message_interaction("A request to get the hdmi ports", |mut i| async move {
        i.contents_from(json!({
            "pact:content-type": "application/json",
            "request": {"jsonrpc": "matching(type, '2.0')", "id": "matching(integer, 0)", "method": "org.rdk.AVInput.1.getInputDevices", "params": {"typeOfInput": "matching(regex, '(HDMI|COMPOSITE)', 'HDMI')"}},
            "requestMetadata": {
                "path": "/jsonrpc"
            },
            "response": [{
                "jsonrpc": "matching(type, '2.0')",
                "id": "matching(integer, 0)",
                "result": {
                    "devices": [
                        {
                            "id": "matching(integer, 0)",
                            "locator": "matching(type, 'hdmiin://localhost/deviceid/0')",
                            "connected": "matching(boolean, true)"
                        }
                    ],
                    "success": "matching(boolean, true)"
                }
            }]
        })).await;
        i.test_name("get_device_hdmi_input");

        i
    }).await;

    let mock_server = pact_builder_async
        .start_mock_server_async(Some("websockets/transport/websockets"))
        .await;

    let payload = ExtnPayload::Request(ExtnRequest::Device(DeviceRequest::Hdmi(
        HdmiRequest::GetAvailableInputs,
    )));
    let msg = get_extn_msg(payload);

    let url = url::Url::parse(mock_server.path("/jsonrpc").as_str()).unwrap();
    let thunder_client = ThunderClientPool::start(url, None, 1).await.unwrap();

    let (s, r) = unbounded();
    let extn_client = get_extn_client(s.clone(), r.clone());

    let state: ThunderState = ThunderState::new(extn_client, thunder_client);

    let _ =
    ThunderHdmiRequestProcessor::process_request(state, msg, HdmiRequest::GetAvailableInputs)
            .await;
}