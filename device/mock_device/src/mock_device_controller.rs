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

use crate::{
    mock_data::MockData,
    mock_device_ffi::EXTN_NAME,
    mock_server::{EmitEventParams, MockServerRequest},
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ripple_sdk::{
    api::gateway::rpc_gateway_api::CallContext,
    async_trait::async_trait,
    extn::{
        client::extn_client::ExtnClient,
        extn_id::{ExtnClassId, ExtnId, ExtnProviderRequest, ExtnProviderResponse},
    },
    log::debug,
    tokio::runtime::Runtime,
    utils::{error::RippleError, rpc_utils::rpc_err},
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

impl From<MockDeviceControllerError> for String {
    fn from(value: MockDeviceControllerError) -> Self {
        value.to_string()
    }
}

#[rpc(server)]
pub trait MockDeviceController {
    #[method(name = "mockdevice.emitEvent")]
    async fn emit_event(
        &self,
        ctx: CallContext,
        req: EmitEventParams,
    ) -> RpcResult<ExtnProviderResponse>;

    #[method(name = "mockdevice.addRequests")]
    async fn add_request_responses(
        &self,
        ctx: CallContext,
        req: MockData,
    ) -> RpcResult<ExtnProviderResponse>;

    #[method(name = "mockdevice.removeRequests")]
    async fn remove_requests(
        &self,
        ctx: CallContext,
        req: MockData,
    ) -> RpcResult<ExtnProviderResponse>;
}

pub struct MockDeviceController {
    client: ExtnClient,
    rt: Runtime,
    id: ExtnId,
}

impl MockDeviceController {
    pub fn new(client: ExtnClient) -> MockDeviceController {
        MockDeviceController {
            client,
            rt: Runtime::new().unwrap(),
            id: ExtnId::new_channel(ExtnClassId::Device, EXTN_NAME.into()),
        }
    }

    async fn request(
        &self,
        request: MockServerRequest,
    ) -> Result<ExtnProviderResponse, MockDeviceControllerError> {
        debug!("request={request:?}");
        let client = self.client.clone();
        let request = ExtnProviderRequest {
            value: serde_json::to_value(request).unwrap(),
            id: self.id.clone(),
        };
        self.rt
            .spawn(async move {
                client
                    .standalone_request(request, 5000)
                    .await
                    .map_err(MockDeviceControllerError::RequestFailed)
            })
            .await
            .map_err(|_| MockDeviceControllerError::ExtnCommunicationFailed)?
    }
}

#[async_trait]
impl MockDeviceControllerServer for MockDeviceController {
    async fn add_request_responses(
        &self,
        _ctx: CallContext,
        req: MockData,
    ) -> RpcResult<ExtnProviderResponse> {
        let res = self
            .request(MockServerRequest::AddRequestResponse(req))
            .await
            .map_err(rpc_err)?;

        Ok(res)
    }

    async fn remove_requests(
        &self,
        _ctx: CallContext,
        req: MockData,
    ) -> RpcResult<ExtnProviderResponse> {
        let res = self
            .request(MockServerRequest::RemoveRequestResponse(req))
            .await
            .map_err(rpc_err)?;

        Ok(res)
    }

    async fn emit_event(
        &self,
        _ctx: CallContext,
        req: EmitEventParams,
    ) -> RpcResult<ExtnProviderResponse> {
        let res = self
            .request(MockServerRequest::EmitEvent(req))
            .await
            .map_err(rpc_err)?;

        Ok(res)
    }
}
