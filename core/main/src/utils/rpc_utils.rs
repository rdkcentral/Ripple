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

use jsonrpsee::core::{Error, RpcResult};
use ripple_sdk::tokio::sync::oneshot;

pub fn rpc_err(msg: impl Into<String>) -> Error {
    Error::Custom(msg.into())
}

/// Awaits a oneshot to respond. If the oneshot fails to repond, creates a generic
/// RPC internal error
pub async fn rpc_await_oneshot<T>(rx: oneshot::Receiver<T>) -> RpcResult<T> {
    match rx.await {
        Ok(v) => Ok(v),
        Err(e) => Err(jsonrpsee::core::error::Error::Custom(format!(
            "Internal failure: {:?}",
            e
        ))),
    }
}
