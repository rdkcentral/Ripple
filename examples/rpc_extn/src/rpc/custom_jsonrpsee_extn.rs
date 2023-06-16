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


use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ripple_sdk::{
    api::gateway::rpc_gateway_api::CallContext, extn::client::extn_client::ExtnClient,
};

pub struct CustomImpl {
    _client: ExtnClient,
}

impl CustomImpl {
    pub fn new(_client: ExtnClient) -> CustomImpl {
        CustomImpl { _client }
    }
}

#[rpc(server)]
pub trait Custom {
    #[method(name = "custom.info")]
    fn info(&self, ctx: CallContext) -> RpcResult<String>;
}

impl CustomServer for CustomImpl {
    fn info(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok("Custom".into())
    }
}
