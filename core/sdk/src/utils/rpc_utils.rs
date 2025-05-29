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

use jsonrpsee::types::{error::CallError, ErrorObject};

use crate::JsonRpcErrorType;

pub fn rpc_err(msg: impl Into<String>) -> JsonRpcErrorType {
    JsonRpcErrorType::Custom(msg.into())
}
pub fn rpc_error_with_code<T>(msg: impl Into<String>, code: i32) -> JsonRpcErrorType {
    JsonRpcErrorType::Call(CallError::Custom(ErrorObject::owned::<()>(
        code,
        msg.into(),
        None,
    )))
}

pub fn rpc_error_with_code_result<T>(
    msg: impl Into<String>,
    code: i32,
) -> Result<T, JsonRpcErrorType> {
    let msg_str = msg.into();
    Err(rpc_error_with_code::<T>(msg_str, code))
}
/*
Legacy function - used to minimally disrupt existing code
*/
pub fn rpc_custom_error_result<T>(msg: impl Into<String>) -> Result<T, JsonRpcErrorType> {
    Err::<T, _>(rpc_custom_error::<T>(msg))
}
pub fn rpc_custom_error<T>(msg: impl Into<String>) -> JsonRpcErrorType {
    JsonRpcErrorType::Custom(msg.into())
}

pub fn extract_tcp_port(url: &str) -> Result<String, JsonRpcErrorType> {
    let url_split: Vec<&str> = url.split("://").collect();
    if let Some(domain) = url_split.get(1) {
        let domain_split: Vec<&str> = domain.split('/').collect();
        if let Some(first_part) = domain_split.first() {
            Ok(first_part.to_string())
        } else {
            rpc_custom_error_result("Invalid domain format")
        }
    } else {
        rpc_custom_error_result("Invalid URL format")
    }
}
