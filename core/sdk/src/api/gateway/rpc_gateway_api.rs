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

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};

use crate::{
    api::firebolt::fb_openrpc::FireboltOpenRpcMethod,
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

#[derive(Debug, Clone, Default)]
pub struct CallerSession {
    pub session_id: Option<String>,
    pub app_id: Option<String>,
}

impl From<CallContext> for CallerSession {
    fn from(ctx: CallContext) -> Self {
        CallerSession {
            session_id: Some(ctx.session_id),
            app_id: Some(ctx.app_id),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppIdentification {
    pub app_id: String,
}

impl From<CallContext> for AppIdentification {
    fn from(ctx: CallContext) -> Self {
        AppIdentification { app_id: ctx.app_id }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct CallContext {
    pub session_id: String,
    pub request_id: String,
    pub app_id: String,
    pub call_id: u64,
    pub protocol: ApiProtocol,
    pub method: String,
    pub cid: Option<String>,
    pub gateway_secure: bool,
}

impl CallContext {
    // TODO: refactor this to use less arguments
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: String,
        request_id: String,
        app_id: String,
        call_id: u64,
        protocol: ApiProtocol,
        method: String,
        cid: Option<String>,
        gateway_secure: bool,
    ) -> CallContext {
        CallContext {
            session_id,
            request_id,
            app_id,
            call_id,
            protocol,
            method,
            cid,
            gateway_secure,
        }
    }

    pub fn get_id(&self) -> String {
        if let Some(cid) = &self.cid {
            return cid.clone();
        }
        self.session_id.clone()
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum ApiProtocol {
    Bridge,
    Extn,
    JsonRpc,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ApiMessage {
    pub protocol: ApiProtocol,
    pub jsonrpc_msg: String,
    pub request_id: String,
}

/// Holds a message in jsonrpc protocol format and the protocol that it should be converted into
/// The protocol of the request is passed in context and then when
/// a response is being written, that protocol is passed here so the transport layers know which protocol
/// should be used for the response.
impl ApiMessage {
    pub fn new(protocol: ApiProtocol, jsonrpc_msg: String, request_id: String) -> ApiMessage {
        ApiMessage {
            protocol,
            jsonrpc_msg,
            request_id,
        }
    }

    pub fn is_error(&self) -> bool {
        // currently only these json rpsee errors are used in Ripple
        self.jsonrpc_msg.contains("Custom error:") || self.jsonrpc_msg.contains("Method not found")
    }

    pub fn get_error_code_from_msg(&self) -> Result<Option<i32>, serde_json::Error> {
        let v: Value = serde_json::from_str(&self.jsonrpc_msg)?;

        if let Some(error) = v.get("error") {
            if let Some(code) = error.get("code") {
                return Ok(Some(code.as_i64().unwrap() as i32));
            }
        }
        // if there is no error code, return None
        Ok(None)
    }
}

#[derive(Deserialize)]
struct ApiBaseRequest {
    jsonrpc: Option<String>,
}

impl ApiBaseRequest {
    fn is_jsonrpc(&self) -> bool {
        self.jsonrpc.is_some()
    }
}

#[derive(Serialize, Deserialize)]
pub struct JsonRpcApiRequest {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Serialize, Deserialize)]
pub struct JsonRpcApiResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RpcRequest {
    pub method: String,
    pub params_json: String,
    pub ctx: CallContext,
}

impl ExtnPayloadProvider for RpcRequest {
    fn get_extn_payload(&self) -> ExtnPayload {
        ExtnPayload::Request(ExtnRequest::Rpc(self.clone()))
    }

    fn get_from_payload(payload: ExtnPayload) -> Option<Self> {
        if let ExtnPayload::Request(ExtnRequest::Rpc(v)) = payload {
            return Some(v);
        }
        None
    }

    fn contract() -> RippleContract {
        RippleContract::Rpc
    }
}

#[derive(Debug)]
pub struct RequestParseError {}

impl RpcRequest {
    pub fn new(method: String, params_json: String, ctx: CallContext) -> RpcRequest {
        RpcRequest {
            method,
            params_json,
            ctx,
        }
    }
    /// Serializes a parameter so that the given ctx becomes the first list in a json array of
    /// parameters. Each rpc handler will get the call context as the first param and
    /// the actual request parameter as the second param.
    ///
    /// # Arguments
    ///
    /// * `req_params` - The request parameter that becomes the second handler parameter
    /// * `ctx` - Context around the call which becomes the first handler parameter
    pub fn prepend_ctx(req_params: Option<Value>, ctx: &CallContext) -> String {
        let mut ps = Vec::<Value>::new();
        ps.push(json!(ctx));
        if let Some(params) = req_params {
            ps.push(json!(params));
        }
        json!(ps).to_string()
    }

    /// Parses a json string into an RpcRequest
    /// Checks if jsonrpc field is present in order to determine
    /// which protocol this message is using.
    /// Parse the different protocol messages into a common request object
    /// # Arguments
    ///
    /// * `json` - The json string to parse
    /// * `app_id` - The app_id this message was from, used to populate the context
    /// * `session_id` - The session_id this message was from, used to populate the context
    pub fn parse(
        json: String,
        app_id: String,
        session_id: String,
        request_id: String,
        cid: Option<String>,
        gateway_secure: bool,
    ) -> Result<RpcRequest, RequestParseError> {
        let parsed_res = serde_json::from_str(&json);
        if parsed_res.is_err() {
            return Err(RequestParseError {});
        }
        let parsed: serde_json::Value = parsed_res.unwrap();
        let base_res = serde_json::from_value(parsed.clone());
        if base_res.is_err() {
            return Err(RequestParseError {});
        }
        let base: ApiBaseRequest = base_res.unwrap();
        if !base.is_jsonrpc() {
            return Err(RequestParseError {});
        }
        let jsonrpc_req_res = serde_json::from_value(parsed);
        if jsonrpc_req_res.is_err() {
            return Err(RequestParseError {});
        }
        let jsonrpc_req: JsonRpcApiRequest = jsonrpc_req_res.unwrap();
        let id = jsonrpc_req.id.unwrap_or(0);
        let method = FireboltOpenRpcMethod::name_with_lowercase_module(&jsonrpc_req.method);
        let ctx = CallContext::new(
            session_id,
            request_id,
            app_id,
            id,
            ApiProtocol::JsonRpc,
            method.clone(),
            cid,
            gateway_secure,
        );
        let ps = RpcRequest::prepend_ctx(jsonrpc_req.params, &ctx);
        Ok(RpcRequest::new(method, ps, ctx))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcGatewayCommand {
    Handle {
        req: String,
        req_id: String,
        ctx: ClientContext,
    },
    Route {
        req: RpcRequest,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientContext {
    pub session_id: String,
    pub app_id: String,
    pub gateway_secure: bool,
}

#[derive(Debug)]
pub enum PermissionCommand {
    GateRequest {
        req: RpcRequest,
        route_tx: oneshot::Sender<bool>,
        session_tx: mpsc::Sender<ApiMessage>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::gateway::rpc_gateway_api::{ApiProtocol, CallContext};
    use crate::utils::test_utils::test_extn_payload_provider;

    #[test]
    fn test_caller_session_from_call_context() {
        let ctx = CallContext {
            session_id: "session123".to_string(),
            request_id: "request123".to_string(),
            app_id: "app123".to_string(),
            call_id: 1,
            protocol: ApiProtocol::Bridge,
            method: "method123".to_string(),
            cid: Some("cid123".to_string()),
            gateway_secure: true,
        };

        let caller_session: CallerSession = ctx.into();

        assert_eq!(caller_session.session_id, Some("session123".to_string()));
        assert_eq!(caller_session.app_id, Some("app123".to_string()));
    }

    #[test]
    fn test_caller_session_from_app_identification() {
        let ctx = CallContext {
            session_id: "session123".to_string(),
            request_id: "request123".to_string(),
            app_id: "app123".to_string(),
            call_id: 1,
            protocol: ApiProtocol::Bridge,
            method: "method123".to_string(),
            cid: Some("cid123".to_string()),
            gateway_secure: true,
        };

        let app_identification: AppIdentification = ctx.into();
        assert_eq!(app_identification.app_id, "app123".to_string());
    }

    #[test]
    fn test_api_message_new() {
        let api_message = ApiMessage::new(
            ApiProtocol::Bridge,
            "jsonrpc_msg".to_string(),
            "request_id".to_string(),
        );

        assert_eq!(api_message.protocol, ApiProtocol::Bridge);
        assert_eq!(api_message.jsonrpc_msg, "jsonrpc_msg".to_string());
        assert_eq!(api_message.request_id, "request_id".to_string());
    }

    #[test]
    fn test_call_context_new() {
        let call_context = CallContext::new(
            "session_id".to_string(),
            "request_id".to_string(),
            "app_id".to_string(),
            1,
            ApiProtocol::Bridge,
            "method".to_string(),
            Some("cid".to_string()),
            true,
        );

        assert_eq!(call_context.session_id, "session_id".to_string());
        assert_eq!(call_context.request_id, "request_id".to_string());
        assert_eq!(call_context.app_id, "app_id".to_string());
        assert_eq!(call_context.call_id, 1);
        assert_eq!(call_context.protocol, ApiProtocol::Bridge);
        assert_eq!(call_context.method, "method".to_string());
        assert_eq!(call_context.cid, Some("cid".to_string()));
        assert!(call_context.gateway_secure);
    }

    #[test]
    fn test_get_id_with_cid() {
        let ctx = CallContext::new(
            "session_id".to_string(),
            "request_id".to_string(),
            "app_id".to_string(),
            1,
            ApiProtocol::Bridge,
            "method".to_string(),
            Some("cid".to_string()),
            true,
        );

        let id = ctx.get_id();
        assert_eq!(id, "cid".to_string());
    }

    #[test]
    fn test_get_id_without_cid() {
        let ctx = CallContext::new(
            "session_id".to_string(),
            "request_id".to_string(),
            "app_id".to_string(),
            1,
            ApiProtocol::Bridge,
            "method".to_string(),
            None,
            true,
        );

        let id = ctx.get_id();
        assert_eq!(id, "session_id".to_string());
    }

    #[test]
    fn test_is_errors() {
        let api_message = ApiMessage {
            protocol: ApiProtocol::Bridge,
            jsonrpc_msg: "Custom error: error".to_string(),
            request_id: "request_id".to_string(),
        };

        assert!(api_message.is_error());
    }

    #[test]
    fn test_api_base_request_is_jsonrpc_with_jsonrpc() {
        let base_request = ApiBaseRequest {
            jsonrpc: Some("2.0".to_string()),
        };

        assert!(base_request.is_jsonrpc());
    }

    #[test]
    fn test_api_base_request_is_jsonrpc_without_jsonrpc() {
        let base_request = ApiBaseRequest { jsonrpc: None };

        assert!(!base_request.is_jsonrpc());
    }

    #[test]
    fn test_rpc_request_new() {
        let method = String::from("test_method");
        let params_json = String::from("test_params");
        let ctx = CallContext::new(
            String::from("test_session_id"),
            String::from("test_request_id"),
            String::from("test_app_id"),
            123,
            ApiProtocol::JsonRpc,
            String::from("test_method"),
            None,
            true,
        );

        let rpc_request = RpcRequest::new(method.clone(), params_json.clone(), ctx.clone());

        assert_eq!(rpc_request.method, method);
        assert_eq!(rpc_request.params_json, params_json);
        assert_eq!(rpc_request.ctx, ctx);
    }

    #[test]
    fn test_rpc_request_prepend_ctx() {
        let req_params = Some(json!({"param1": "value1"}));
        let ctx = CallContext::new(
            String::from("test_session_id"),
            String::from("test_request_id"),
            String::from("test_app_id"),
            123,
            ApiProtocol::JsonRpc,
            String::from("test_method"),
            None,
            true,
        );

        let result = RpcRequest::prepend_ctx(req_params.clone(), &ctx);

        let expected_result = json!([ctx, req_params]).to_string();
        assert_eq!(result, expected_result);
    }

    // #[test]
    // fn test_rpc_request_parse() {
    //     let json = String::from(
    //         r#"{"jsonrpc": "2.0", "id": 123, "method": "test_method", "params": {"param1": "value1"}}"#,
    //     );
    //     let app_id = String::from("test_app_id");
    //     let session_id = String::from("test_session_id");
    //     let request_id = String::from("test_request_id");
    //     let cid = None;
    //     let gateway_secure = true;

    //     let result = RpcRequest::parse(json, app_id, session_id, request_id, cid, gateway_secure);

    //     assert!(result.is_ok());
    //     let rpc_request: RpcRequest = result.unwrap();

    //     // Parse the `params_json` string from the `RpcRequest` into a `serde_json::Value`
    //     let actual_params_val: Value = serde_json::from_str(&rpc_request.params_json)
    //         .expect("Failed to parse JSON from params_json string");

    //     // Define the expected JSON structure as a `serde_json::Value`
    //     // Note: Adjust the structure below according to the actual expected content of `params_json`
    //     let expected_params_val: Value = json!({
    //         "app_id": rpc_request.ctx.app_id,
    //         "call_id": rpc_request.ctx.call_id,
    //         "cid": rpc_request.ctx.cid,
    //         "gateway_secure": rpc_request.ctx.gateway_secure,
    //         "method": rpc_request.ctx.method,
    //         "protocol": rpc_request.ctx.protocol.
    //         //"request_id": rpc_request.ctx.request_id,
    //         "session_id": rpc_request.ctx.session_id,
    //         "params": {
    //             "param1": "value1"
    //         }
    //     });

    //     // Compare the actual and expected `serde_json::Value` instances
    //     assert_eq!(actual_params_val, expected_params_val);
    //     assert_eq!(rpc_request.ctx.session_id, String::from("test_session_id"));
    //     assert_eq!(rpc_request.ctx.request_id, String::from("test_request_id"));
    //     assert_eq!(rpc_request.ctx.app_id, String::from("test_app_id"));
    //     assert_eq!(rpc_request.ctx.protocol, ApiProtocol::JsonRpc);
    //     assert_eq!(rpc_request.ctx.method, String::from("test_method"));
    //     assert_eq!(rpc_request.ctx.cid, None);
    //     assert!(rpc_request.ctx.gateway_secure);
    // }
    #[test]
    fn test_extn_request_rpc() {
        let call_context = CallContext {
            session_id: "test_session_id".to_string(),
            request_id: "test_request_id".to_string(),
            app_id: "test_app_id".to_string(),
            call_id: 123,
            protocol: ApiProtocol::Bridge,
            method: "some_method".to_string(),
            cid: Some("some_cid".to_string()),
            gateway_secure: true,
        };

        let rpc_request = RpcRequest {
            method: "some_method".to_string(),
            params_json: r#"{"key": "value"}"#.to_string(),
            ctx: call_context,
        };
        let contract_type: RippleContract = RippleContract::Rpc;
        test_extn_payload_provider(rpc_request, contract_type);
    }

    #[test]
    fn test_get_error_code_from_msg() {
        let api_message = ApiMessage {
            protocol: ApiProtocol::JsonRpc,
            jsonrpc_msg: r#"{"jsonrpc": "2.0", "id": 123, "error": {"code": 456, "message": "error message"}}"#.to_string(),
            request_id: "request_id".to_string(),
        };

        let result = api_message.get_error_code_from_msg();

        assert!(result.is_ok());
        let error_code = result.unwrap();
        assert_eq!(error_code, Some(456));
    }

    #[test]
    fn test_get_error_code_from_msg_error_code_not_present() {
        let api_message = ApiMessage {
            protocol: ApiProtocol::JsonRpc,
            jsonrpc_msg: r#"{"jsonrpc": "2.0", "id": 123, "error": {"message": "error message"}}"#
                .to_string(),
            request_id: "request_id".to_string(),
        };

        let result = api_message.get_error_code_from_msg();

        assert!(result.is_ok());
        let error_code = result.unwrap();
        assert_eq!(error_code, None);
    }

    #[test]
    fn test_get_error_code_from_msg_result_present() {
        let api_message = ApiMessage {
            protocol: ApiProtocol::JsonRpc,
            jsonrpc_msg: r#"{"jsonrpc": "2.0", "id": 123, "result": null}"#.to_string(),
            request_id: "request_id".to_string(),
        };

        let result = api_message.get_error_code_from_msg();

        assert!(result.is_ok());
        let error_code = result.unwrap();
        assert_eq!(error_code, None);
    }
}
