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

use std::fmt::Display;

use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ripple_sdk::{
    api::{
        gateway::rpc_gateway_api::CallContext,
        mock_server::{
            AddRequestResponseParams, EmitEventParams, MockServerRequest, MockServerResponse,
            RemoveRequestParams,
        },
    },
    async_trait::async_trait,
    extn::client::extn_client::ExtnClient,
    log::debug,
    tokio::runtime::Runtime,
    utils::error::RippleError,
};

#[derive(Debug, Clone)]
enum MockDeviceControllerError {
    RequestFailed(RippleError),
    ExtnCommunicationFailed,
}

impl std::error::Error for MockDeviceControllerError {}

impl Display for MockDeviceControllerError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::result::Result<(), ::std::fmt::Error> {
        let msg = match self.clone() {
            MockDeviceControllerError::RequestFailed(err) => {
                format!("Failed to complete the request. RippleError {err:?}")
            }
            MockDeviceControllerError::ExtnCommunicationFailed => {
                "Failed to communicate with the Mock Device extension".to_owned()
            }
        };

        f.write_str(msg.as_str())
    }
}

#[rpc(server)]
pub trait MockDeviceController {
    #[method(name = "mockdevice.addRequestResponse")]
    async fn add_request_response(
        &self,
        ctx: CallContext,
        req: AddRequestResponseParams,
    ) -> RpcResult<MockServerResponse>;

    #[method(name = "mockdevice.removeRequest")]
    async fn remove_request(
        &self,
        ctx: CallContext,
        req: RemoveRequestParams,
    ) -> RpcResult<MockServerResponse>;

    #[method(name = "mockdevice.emitEvent")]
    async fn emit_event(
        &self,
        ctx: CallContext,
        req: EmitEventParams,
    ) -> RpcResult<MockServerResponse>;
}

pub struct MockDeviceController {
    client: ExtnClient,
    rt: Runtime,
}

impl MockDeviceController {
    pub fn new(client: ExtnClient) -> MockDeviceController {
        MockDeviceController {
            client,
            rt: Runtime::new().unwrap(),
        }
    }

    async fn request(
        &self,
        request: MockServerRequest,
    ) -> Result<MockServerResponse, MockDeviceControllerError> {
        debug!("request={request:?}");
        let mut client = self.client.clone();
        self.rt
            .spawn(async move {
                client
                    .standalone_request(request, 5000)
                    .await
                    .map_err(MockDeviceControllerError::RequestFailed)
            })
            .await
            .map_err(|_e| MockDeviceControllerError::ExtnCommunicationFailed)?
    }
}

#[async_trait]
impl MockDeviceControllerServer for MockDeviceController {
    async fn add_request_response(
        &self,
        _ctx: CallContext,
        req: AddRequestResponseParams,
    ) -> RpcResult<MockServerResponse> {
        let res = self
            .request(MockServerRequest::AddRequestResponse(req))
            .await
            .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;

        Ok(res)
    }

    async fn remove_request(
        &self,
        _ctx: CallContext,
        req: RemoveRequestParams,
    ) -> RpcResult<MockServerResponse> {
        let res = self
            .request(MockServerRequest::RemoveRequest(req))
            .await
            .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;

        Ok(res)
    }

    async fn emit_event(
        &self,
        _ctx: CallContext,
        req: EmitEventParams,
    ) -> RpcResult<MockServerResponse> {
        let res = self
            .request(MockServerRequest::EmitEvent(req))
            .await
            .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;

        Ok(res)
    }
}
