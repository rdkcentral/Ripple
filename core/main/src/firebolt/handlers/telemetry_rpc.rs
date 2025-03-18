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

use jsonrpsee::{core::RpcResult, proc_macros::rpc, RpcModule};
use ripple_sdk::{
    api::{firebolt::fb_telemetry::TelemetryPayload, gateway::rpc_gateway_api::CallContext},
    async_trait::async_trait,
};

use crate::{
    firebolt::rpc::RippleRPCProvider, service::telemetry_builder::TelemetryBuilder,
    state::platform_state::PlatformState,
};

#[rpc(server)]
pub trait Telemetry {
    #[method(name = "ripple.sendTelemetry")]
    async fn send_telemetry(&self, ctx: CallContext, payload: TelemetryPayload) -> RpcResult<()>;

    #[method(name = "ripple.setTelemetrySessionId")]
    fn set_telemetry_session_id(&self, ctx: CallContext, session_id: String) -> RpcResult<()>;
}

#[derive(Debug)]
pub struct TelemetryImpl {
    pub state: PlatformState,
}

#[async_trait]
impl TelemetryServer for TelemetryImpl {
    async fn send_telemetry(&self, _ctx: CallContext, payload: TelemetryPayload) -> RpcResult<()> {
        let _ = TelemetryBuilder::send_telemetry(&self.state, payload);
        Ok(())
    }

    fn set_telemetry_session_id(&self, _ctx: CallContext, session_id: String) -> RpcResult<()> {
        self.state.otel.update_session_id(Some(session_id));
        Ok(())
    }
}

pub struct TelemetryProvider;
impl RippleRPCProvider<TelemetryImpl> for TelemetryProvider {
    fn provide(state: PlatformState) -> RpcModule<TelemetryImpl> {
        (TelemetryImpl { state }).into_rpc()
    }
}
