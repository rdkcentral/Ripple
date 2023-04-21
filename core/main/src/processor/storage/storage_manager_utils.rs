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
use ripple_sdk::{
    api::device::device_accessibility_data::StorageData,
    extn::extn_client_message::ExtnResponse,
    serde_json::{self, Value},
    utils::error::RippleError,
};

fn storage_error() -> jsonrpsee::core::Error {
    Error::Custom(String::from("error parsing response"))
}

fn get_storage_data(resp: Result<ExtnResponse, RippleError>) -> Result<Option<StorageData>, Error> {
    if let Err(_) = resp {
        return Err(storage_error());
    }
    match resp.unwrap() {
        ExtnResponse::StorageData(storage_data) => Ok(Some(storage_data)),
        _ => Ok(None),
    }
}

fn get_value(resp: Result<ExtnResponse, RippleError>) -> Result<Value, Error> {
    let has_storage_data = get_storage_data(resp.clone())?;

    if let Some(storage_data) = has_storage_data {
        return Ok(storage_data.value);
    }

    // // K/V stored prior to StorageData implementation.
    // match resp.unwrap().as_value() {
    //     Some(value) => Ok(value),
    //     None => Err(storage_error()),
    // }

    match resp.unwrap() {
        ExtnResponse::Value(value) => Ok(value.clone()),
        ExtnResponse::String(str_val) => match serde_json::from_str(&str_val) {
            Ok(value) => Ok(value),
            Err(_) => Err(storage_error()),
        },
        ExtnResponse::None(()) | _ => Err(storage_error()),
    }
}

pub fn storage_to_string_rpc_result(resp: Result<ExtnResponse, RippleError>) -> RpcResult<String> {
    let value = get_value(resp.clone())?;

    if let Some(s) = value.as_str() {
        return Ok(s.to_string());
    }

    Err(storage_error())
}

pub fn storage_to_bool_rpc_result(resp: Result<ExtnResponse, RippleError>) -> RpcResult<bool> {
    let value = get_value(resp)?;

    if let Some(b) = value.as_bool() {
        return Ok(b);
    }
    if let Some(s) = value.as_str() {
        return Ok(s == "true");
    }

    Err(storage_error())
}

pub fn storage_to_u32_rpc_result(resp: Result<ExtnResponse, RippleError>) -> RpcResult<u32> {
    let value = get_value(resp)?;
    if let Some(n) = value.as_u64() {
        return Ok(n as u32);
    }
    if let Some(s) = value.as_str() {
        return s.parse::<u32>().map_or(Err(storage_error()), |v| Ok(v));
    }

    Err(storage_error())
}

pub fn storage_to_f32_rpc_result(resp: Result<ExtnResponse, RippleError>) -> RpcResult<f32> {
    let value = get_value(resp)?;

    if let Some(n) = value.as_f64() {
        return Ok(n as f32);
    }
    if let Some(s) = value.as_str() {
        return s
            .parse::<f64>()
            .map_or(Err(storage_error()), |v| Ok(v as f32));
    }

    Err(storage_error())
}

pub fn storage_to_void_rpc_result(resp: Result<ExtnResponse, RippleError>) -> RpcResult<()> {
    match resp {
        Ok(_) => Ok(()),
        Err(_) => Err(storage_error()),
    }
}
