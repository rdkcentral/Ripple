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
    mock_data::MockDeviceState,
    mock_server::{
        AddRequestResponseResponse, EmitEventParams, EmitEventResponse, RemoveRequestResponse,
    },
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ripple_sdk::{
    api::gateway::rpc_gateway_api::CallContext,
    async_trait::async_trait,
    extn::extn_id::ExtnProviderResponse,
    utils::{error::RippleError, rpc_utils::rpc_err},
};

#[derive(Debug, Clone)]
enum MockDeviceControllerError {
    RequestFailed(RippleError),
}

impl std::error::Error for MockDeviceControllerError {}

impl Display for MockDeviceControllerError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::result::Result<(), ::std::fmt::Error> {
        let msg = match self.clone() {
            MockDeviceControllerError::RequestFailed(err) => {
                format!("Failed to complete the request. RippleError {err:?}")
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
    state: MockDeviceState,
}

impl MockDeviceController {
    pub fn new(state: MockDeviceState) -> MockDeviceController {
        MockDeviceController { state }
    }
}

#[async_trait]
impl MockDeviceControllerServer for MockDeviceController {
    async fn add_request_responses(
        &self,
        _ctx: CallContext,
        req: MockData,
    ) -> RpcResult<ExtnProviderResponse> {
        if self
            .state
            .server
            .add_request_response_v2(req)
            .await
            .is_err()
        {
            return Err(rpc_err(MockDeviceControllerError::RequestFailed(
                RippleError::InvalidInput,
            )));
        } else {
            Ok(ExtnProviderResponse {
                value: serde_json::to_value(AddRequestResponseResponse {
                    success: true,
                    error: None,
                })
                .unwrap(),
            })
        }
    }

    async fn remove_requests(
        &self,
        _ctx: CallContext,
        req: MockData,
    ) -> RpcResult<ExtnProviderResponse> {
        if self
            .state
            .server
            .remove_request_response_v2(req)
            .await
            .is_err()
        {
            return Err(rpc_err(MockDeviceControllerError::RequestFailed(
                RippleError::InvalidInput,
            )));
        } else {
            Ok(ExtnProviderResponse {
                value: serde_json::to_value(RemoveRequestResponse {
                    success: true,
                    error: None,
                })
                .unwrap(),
            })
        }
    }

    async fn emit_event(
        &self,
        _ctx: CallContext,
        req: EmitEventParams,
    ) -> RpcResult<ExtnProviderResponse> {
        self.state
            .server
            .emit_event(&req.event.body, req.event.delay)
            .await;
        Ok(ExtnProviderResponse {
            value: serde_json::to_value(EmitEventResponse { success: true }).unwrap(),
        })
    }
}
