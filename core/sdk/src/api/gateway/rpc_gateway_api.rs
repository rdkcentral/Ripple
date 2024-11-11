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

use chrono::Utc;
use log::debug;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::{
    api::firebolt::{fb_general::ListenRequest, fb_openrpc::FireboltOpenRpcMethod},
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct CallContext {
    pub session_id: String,
    pub request_id: String,
    pub app_id: String,
    pub call_id: u64,
    pub protocol: ApiProtocol,
    pub method: String,
    pub cid: Option<String>,
    pub gateway_secure: bool,
    pub context: Vec<String>,
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
            context: Vec::new(),
        }
    }

    pub fn get_id(&self) -> String {
        if let Some(cid) = &self.cid {
            return cid.clone();
        }
        self.session_id.clone()
    }

    pub fn is_event_based(&self) -> bool {
        self.context.contains(&"eventBased".to_owned())
    }
}

impl crate::Mockable for CallContext {
    fn mock() -> Self {
        CallContext {
            session_id: "session_id".to_owned(),
            request_id: "1".to_owned(),
            app_id: "some_app_id".to_owned(),
            call_id: 1,
            protocol: ApiProtocol::JsonRpc,
            method: "module.method".to_owned(),
            cid: Some("cid".to_owned()),
            gateway_secure: true,
            context: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub enum ApiProtocol {
    Bridge,
    Extn,
    #[default]
    JsonRpc,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ApiMessage {
    pub protocol: ApiProtocol,
    pub jsonrpc_msg: String,
    pub request_id: String,
    pub stats: Option<ApiStats>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ApiStats {
    pub stats_ref: String,
    pub stats: RpcStats,
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
            stats: None,
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

impl JsonRpcApiRequest {
    pub fn new(method: String, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: None,
            method,
            params,
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct JsonRpcApiError {
    pub code: i32,
    pub id: Option<u64>,
    pub message: String,
    pub method: Option<String>,
    pub params: Option<Value>,
}
impl JsonRpcApiError {
    pub fn new(
        code: i32,
        id: Option<u64>,
        message: String,
        method: Option<String>,
        params: Option<Value>,
    ) -> Self {
        JsonRpcApiError {
            code,
            id,
            message,
            method,
            params,
        }
    }
    pub fn with_method(mut self, method: String) -> Self {
        self.method = Some(method);
        self
    }
    pub fn with_params(mut self, params: Option<Value>) -> Self {
        self.params = params;
        self
    }
    pub fn with_id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }
    pub fn with_message(mut self, message: String) -> Self {
        self.message = message;
        self
    }
    pub fn with_code(mut self, code: i32) -> Self {
        self.code = code;
        self
    }
    pub fn to_response(&self) -> JsonRpcApiResponse {
        JsonRpcApiResponse::error(self)
    }
}
impl From<JsonRpcApiError> for JsonRpcApiResponse {
    fn from(error: JsonRpcApiError) -> Self {
        JsonRpcApiResponse::error(&error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcApiResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Default for JsonRpcApiResponse {
    fn default() -> Self {
        JsonRpcApiResponse {
            id: None,
            jsonrpc: "2.0".to_string(),
            result: None,
            error: None,
            method: None,
            params: None,
        }
    }
}

impl JsonRpcApiResponse {
    pub fn update_event_message(&mut self, request: &RpcRequest) {
        if request.is_event_based() {
            self.params = self.result.take();
            self.id = None;
            self.method = Some(format!("{}.{}", request.ctx.method, request.ctx.call_id))
        } else {
            self.method = None;
            self.params = None;
        }
    }

    pub fn error(error: &JsonRpcApiError) -> Self {
        JsonRpcApiResponse {
            jsonrpc: "2.0".to_owned(),
            id: error.id,
            result: None,
            error: Some(json!({"code": error.code, "message": error.message})),
            method: error.method.clone(),
            params: error.params.clone(),
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        serde_json::to_string(self).unwrap().as_bytes().to_vec()
    }
    pub fn with_result(mut self, result: Option<Value>) -> Self {
        self.result = result;
        self.error = None;
        self
    }
    pub fn with_method(mut self, method: Option<String>) -> Self {
        self.method = method;
        self
    }
    pub fn with_params(mut self, params: Option<Value>) -> Self {
        self.params = params;
        self
    }
    pub fn with_id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }
    pub fn with_error(mut self, error: Value) -> Self {
        self.error = Some(error);
        self.result = None;
        self
    }
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
    pub fn is_success(&self) -> bool {
        self.result.is_some()
    }

    pub fn is_response(&self) -> bool {
        self.params.is_none()
            && self.method.is_none()
            && self.id.is_some()
            && (self.result.is_some() || self.error.is_some())
    }

    pub fn get_response(request: &str) -> Option<JsonRpcApiResponse> {
        if let Ok(response) = serde_json::from_str::<JsonRpcApiResponse>(request) {
            if response.is_response() {
                return Some(response);
            }
        }
        None
    }
}

impl crate::Mockable for JsonRpcApiResponse {
    fn mock() -> Self {
        JsonRpcApiResponse {
            jsonrpc: "2.0".to_owned(),
            result: None,
            id: None,
            error: None,
            method: None,
            params: None,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RpcStats {
    pub start_time: i64,
    pub last_stage: i64,
    stage_durations: String,
}

impl Default for RpcStats {
    fn default() -> Self {
        Self {
            start_time: Utc::now().timestamp_millis(),
            last_stage: 0,
            stage_durations: String::new(),
        }
    }
}

impl RpcStats {
    pub fn update_stage(&mut self, stage: &str) -> i64 {
        let current_time = Utc::now().timestamp_millis();
        let mut last_stage = self.last_stage;
        if last_stage == 0 {
            last_stage = self.start_time;
        }
        self.last_stage = current_time;
        let duration = current_time - last_stage;
        if self.stage_durations.is_empty() {
            self.stage_durations = format!("{}={}", stage, duration);
        } else {
            self.stage_durations = format!("{},{}={}", self.stage_durations, stage, duration);
        }
        duration
    }

    pub fn get_total_time(&self) -> i64 {
        let current_time = Utc::now().timestamp_millis();
        current_time - self.start_time
    }

    pub fn get_stage_durations(&self) -> String {
        self.stage_durations.clone()
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct RpcRequest {
    pub method: String,
    pub params_json: String,
    pub ctx: CallContext,
    pub stats: RpcStats,
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

impl crate::Mockable for RpcRequest {
    fn mock() -> Self {
        RpcRequest {
            method: "module.method".to_owned(),
            params_json: "{}".to_owned(),
            ctx: CallContext::mock(),
            stats: RpcStats::default(),
        }
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
            stats: RpcStats::default(),
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
        context: Vec<String>,
    ) -> Result<RpcRequest, RequestParseError> {
        let parsed =
            serde_json::from_str::<serde_json::Value>(&json).map_err(|_| RequestParseError {})?;
        let base = serde_json::from_value::<ApiBaseRequest>(parsed.clone())
            .map_err(|_| RequestParseError {})?;
        if !base.is_jsonrpc() {
            return Err(RequestParseError {});
        }
        let jsonrpc_req = serde_json::from_value::<JsonRpcApiRequest>(parsed)
            .map_err(|_| RequestParseError {})?;

        let id = jsonrpc_req.id.unwrap_or(0);
        let method = FireboltOpenRpcMethod::name_with_lowercase_module(&jsonrpc_req.method);
        let mut ctx = CallContext::new(
            session_id,
            request_id,
            app_id,
            id,
            ApiProtocol::JsonRpc,
            method.clone(),
            cid,
            gateway_secure,
        );
        ctx.context = context;
        let ps = RpcRequest::prepend_ctx(jsonrpc_req.params, &ctx);
        Ok(RpcRequest::new(method, ps, ctx))
    }

    pub fn is_subscription(&self) -> bool {
        self.method.contains(".on") && self.params_json.contains("listen")
    }

    pub fn is_listening(&self) -> bool {
        if let Some(params) = self.get_params() {
            debug!("Successfully got params {:?}", params);
            if let Ok(v) = serde_json::from_value::<ListenRequest>(params) {
                debug!("Successfully got listen request {:?}", v);
                return v.listen;
            }
        }
        false
    }

    pub fn get_unsubscribe(&self) -> RpcRequest {
        let mut rpc_request = self.clone();
        rpc_request.params_json = serde_json::to_string(&ListenRequest { listen: false }).unwrap();
        rpc_request
    }

    pub fn get_params(&self) -> Option<Value> {
        if let Ok(mut v) = serde_json::from_str::<Vec<Value>>(&self.params_json) {
            if v.len() > 1 {
                return v.pop();
            }
        }
        None
    }

    pub fn get_new_internal(method: String, params: Option<Value>) -> Self {
        let ctx = CallContext::new(
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            "internal".into(),
            1,
            crate::api::gateway::rpc_gateway_api::ApiProtocol::Extn,
            method.clone(),
            None,
            false,
        );
        let request = serde_json::to_value(JsonRpcApiRequest::new(method.clone(), params)).unwrap();
        RpcRequest {
            params_json: Self::prepend_ctx(Some(request), &ctx),
            ctx,
            method,
            stats: RpcStats::default(),
        }
    }

    pub fn is_event_based(&self) -> bool {
        self.ctx.is_event_based()
    }

    pub fn add_context(&mut self, context: Vec<String>) {
        self.ctx.context.extend(context)
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
    use crate::Mockable;

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
            context: Vec::new(),
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
            context: Vec::new(),
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
        let api_message = ApiMessage::new(
            ApiProtocol::Bridge,
            "Custom error: error".to_string(),
            "request_id".to_string(),
        );

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
            context: Vec::new(),
        };

        let rpc_request = RpcRequest {
            method: "some_method".to_string(),
            params_json: r#"{"key": "value"}"#.to_string(),
            ctx: call_context,
            stats: RpcStats::default(),
        };
        let contract_type: RippleContract = RippleContract::Rpc;
        test_extn_payload_provider(rpc_request, contract_type);
    }

    #[test]
    fn test_get_error_code_from_msg() {
        let api_message = ApiMessage::new(
            ApiProtocol::JsonRpc,
            r#"{"jsonrpc": "2.0", "id": 123, "error": {"code": 456, "message": "error message"}}"#
                .to_string(),
            "request_id".to_string(),
        );

        let result = api_message.get_error_code_from_msg();

        assert!(result.is_ok());
        let error_code = result.unwrap();
        assert_eq!(error_code, Some(456));
    }

    #[test]
    fn test_get_error_code_from_msg_error_code_not_present() {
        let api_message = ApiMessage::new(
            ApiProtocol::JsonRpc,
            r#"{"jsonrpc": "2.0", "id": 123, "error": {"message": "error message"}}"#.to_string(),
            "request_id".to_string(),
        );

        let result = api_message.get_error_code_from_msg();

        assert!(result.is_ok());
        let error_code = result.unwrap();
        assert_eq!(error_code, None);
    }

    #[test]
    fn test_get_error_code_from_msg_result_present() {
        let api_message = ApiMessage::new(
            ApiProtocol::JsonRpc,
            r#"{"jsonrpc": "2.0", "id": 123, "result": null}"#.to_string(),
            "request_id".to_string(),
        );

        let result = api_message.get_error_code_from_msg();

        assert!(result.is_ok());
        let error_code = result.unwrap();
        assert_eq!(error_code, None);
    }

    #[test]
    fn new_json_rpc_methods() {
        let request = JsonRpcApiRequest::new("some_method".to_owned(), None);
        assert!(request.method.eq("some_method"));
    }

    #[test]
    fn test_rpc_unsubscribe() {
        let new = RpcRequest::mock().get_unsubscribe();
        let request = serde_json::from_str::<ListenRequest>(&new.params_json).unwrap();
        assert!(!request.listen);
    }
}
