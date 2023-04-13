use crate::helpers::error_util::RippleError;
use dab::core::{message::DabResponsePayload, model::persistent_store::StorageData};
use jsonrpsee::core::{Error, RpcResult};
use serde_json::Value;

fn storage_error() -> jsonrpsee::core::Error {
    Error::Custom(String::from("error parsing response"))
}

fn get_storage_data(
    resp: Result<DabResponsePayload, RippleError>,
) -> Result<Option<StorageData>, Error> {
    if let Err(_) = resp {
        return Err(storage_error());
    }
    match resp.unwrap() {
        DabResponsePayload::StorageData(storage_data) => Ok(Some(storage_data)),
        _ => Ok(None),
    }
}

fn get_value(resp: Result<DabResponsePayload, RippleError>) -> Result<Value, Error> {
    let has_storage_data = get_storage_data(resp.clone())?;

    if let Some(storage_data) = has_storage_data {
        return Ok(storage_data.value);
    }

    // K/V stored prior to StorageData implementation.
    match resp.unwrap().as_value() {
        Some(value) => Ok(value),
        None => Err(storage_error()),
    }
}

pub fn storage_to_string_rpc_result(
    resp: Result<DabResponsePayload, RippleError>,
) -> RpcResult<String> {
    let value = get_value(resp.clone())?;

    if let Some(s) = value.as_str() {
        return Ok(s.to_string());
    }

    Err(storage_error())
}

pub fn storage_to_bool_rpc_result(
    resp: Result<DabResponsePayload, RippleError>,
) -> RpcResult<bool> {
    let value = get_value(resp)?;

    if let Some(b) = value.as_bool() {
        return Ok(b);
    }
    if let Some(s) = value.as_str() {
        return Ok(s == "true");
    }

    Err(storage_error())
}

pub fn storage_to_u32_rpc_result(resp: Result<DabResponsePayload, RippleError>) -> RpcResult<u32> {
    let value = get_value(resp)?;
    if let Some(n) = value.as_u64() {
        return Ok(n as u32);
    }
    if let Some(s) = value.as_str() {
        return s.parse::<u32>().map_or(Err(storage_error()), |v| Ok(v));
    }

    Err(storage_error())
}

pub fn storage_to_f32_rpc_result(resp: Result<DabResponsePayload, RippleError>) -> RpcResult<f32> {
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

pub fn storage_to_void_rpc_result(resp: Result<DabResponsePayload, RippleError>) -> RpcResult<()> {
    match resp {
        Ok(_) => Ok(()),
        Err(_) => Err(storage_error()),
    }
}
