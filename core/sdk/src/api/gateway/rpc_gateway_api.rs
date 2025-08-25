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

use jsonrpsee::types::ErrorObject;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::{
    api::{
        firebolt::{fb_general::ListenRequest, fb_openrpc::FireboltOpenRpcMethod},
        observability::metrics_util::ApiStats,
    },
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::RippleContract,
};

pub const RPC_V2: &str = "rpc_v2";

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
impl From<CallContext> for serde_json::Value {
    fn from(ctx: CallContext) -> Self {
        json!({
            "session_id": ctx.session_id,
            "request_id": ctx.request_id,
            "app_id": ctx.app_id,
            "call_id": ctx.call_id,
            "protocol": ctx.protocol,
            "method": ctx.method,
            "cid": ctx.cid,
            "gateway_secure": ctx.gateway_secure,
            "context": ctx.context,
        })
    }
}
impl std::fmt::Display for CallContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "session_id={}, request_id={}, app_id={}, call_id={}, protocol={}, method={}, cid={}",
            self.session_id,
            self.request_id,
            self.app_id,
            self.call_id,
            self.protocol,
            self.method,
            self.cid.as_ref().unwrap_or(&"no_cid".to_string())
        )
    }
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

    pub fn is_rpc_v2(&self) -> bool {
        self.context.contains(&RPC_V2.to_owned())
    }

    pub fn internal(method: &str) -> Self {
        CallContext::new(
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            "internal".into(),
            1,
            ApiProtocol::Extn,
            method.to_owned(),
            None,
            false,
        )
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
    Service,
}
impl std::fmt::Display for ApiProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiProtocol::Bridge => write!(f, "Bridge"),
            ApiProtocol::Extn => write!(f, "Extn"),
            ApiProtocol::JsonRpc => write!(f, "JsonRpc"),
            ApiProtocol::Service => write!(f, "Service"),
        }
    }
}
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ApiMessage {
    pub protocol: ApiProtocol,
    pub jsonrpc_msg: String,
    pub request_id: String,
    pub stats: Option<ApiStats>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn with_id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }
    pub fn as_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}
impl From<JsonRpcApiRequest> for RpcRequest {
    fn from(request: JsonRpcApiRequest) -> Self {
        RpcRequest {
            ctx: CallContext::default(),
            method: request.method,
            params_json: serde_json::to_string(&request.params.unwrap_or_default())
                .unwrap_or_default(),
        }
    }
}
impl From<RpcRequest> for JsonRpcApiRequest {
    fn from(request: RpcRequest) -> Self {
        JsonRpcApiRequest {
            jsonrpc: "2.0".to_owned(),
            id: Some(request.clone().ctx.call_id),
            method: request.clone().method,
            params: request.get_params_as_map(),
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
impl<'a> From<JsonRpcApiError> for ErrorObject<'a> {
    fn from(error: JsonRpcApiError) -> Self {
        ErrorObject::owned(error.code, error.message, error.params)
    }
}
use jsonrpsee::core::Error as JsonRpSeeError;
//use jsonrpsee_core::Error as JsonRpseeError;
impl From<JsonRpcApiError> for JsonRpSeeError {
    fn from(error: JsonRpcApiError) -> Self {
        JsonRpSeeError::Custom(error.message.clone())
        //ErrorObject::owned(error.code, error.message.clone(), error.params.clone())
    }
}
impl From<JsonRpcApiError> for JsonRpcApiResponse {
    fn from(error: JsonRpcApiError) -> Self {
        JsonRpcApiResponse::error(&error)
    }
}

pub fn rpc_value_result_to_string_result(
    result: jsonrpsee::core::RpcResult<Value>,
    default: Option<String>,
) -> jsonrpsee::core::RpcResult<String> {
    match result {
        Ok(v) => Ok(v
            .as_str()
            .unwrap_or(&default.unwrap_or_default())
            .to_string()),
        Err(e) => Err(e),
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
impl From<RpcRequest> for JsonRpcApiResponse {
    fn from(request: RpcRequest) -> Self {
        JsonRpcApiResponse {
            jsonrpc: "2.0".to_owned(),
            id: Some(request.ctx.call_id),
            result: Some(json!({"result": "success"})),
            error: None,
            method: Some(request.clone().method),
            params: request.get_params(),
        }
    }
}

impl JsonRpcApiResponse {
    pub fn new(id: Option<u64>, error: Option<Value>) -> Self {
        Self {
            id,
            jsonrpc: "2.0".to_owned(),
            result: None,
            error,
            method: None,
            params: None,
        }
    }
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value::<JsonRpcApiResponse>(value)
    }

    pub fn update_event_message(&mut self, request: &RpcRequest) {
        if request.is_rpc_v2() {
            self.params = self.result.take();
            self.id = None;
            self.method = Some(request.ctx.method.clone());
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

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct RpcRequest {
    pub method: String,
    pub params_json: String,
    pub ctx: CallContext,
}
impl RpcRequest {
    pub fn internal(method: &str, on_behalf_of: Option<CallContext>) -> Self {
        // This is particularly useful when we need to make an internal/intermediate call
        // on behalf of an app, e.g. for subscriptions.
        let ctx = match on_behalf_of {
            Some(ctx) => CallContext::new(
                ctx.session_id,
                ctx.request_id,
                ctx.app_id,
                ctx.call_id,
                ApiProtocol::Extn,
                method.to_owned(),
                ctx.cid,
                ctx.gateway_secure,
            ),
            None => CallContext::internal(method),
        };

        RpcRequest {
            params_json: Self::prepend_ctx(None, &ctx),
            ctx,
            method: method.to_owned(),
        }
    }
    pub fn with_params(mut self, params: Option<Value>) -> Self {
        self.params_json = Self::prepend_ctx(params, &self.ctx);
        self
    }
    pub fn with_method(mut self, method: String) -> Self {
        self.method = method;
        self
    }
    pub fn with_context(mut self, context: Vec<String>) -> Self {
        self.ctx.context.extend(context);
        self
    }
    pub fn with_cid(mut self, cid: String) -> Self {
        self.ctx.cid = Some(cid);
        self
    }
    pub fn as_json_rpc(&self) -> JsonRpcApiRequest {
        JsonRpcApiRequest {
            jsonrpc: "2.0".to_owned(),
            id: Some(self.ctx.call_id),
            method: self.method.clone(),
            params: self.get_params_as_map(),
        }
    }
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
        self.method.contains(".on")
            && if let Some(params) = self.get_params() {
                return serde_json::from_value::<ListenRequest>(params.clone()).is_ok();
            } else {
                return false;
            }
    }
    pub fn is_unlisten(&self) -> bool {
        self.is_subscription() && !self.is_listening()
    }

    pub fn is_listening(&self) -> bool {
        serde_json::from_value::<ListenRequest>(
            self.get_params().unwrap_or(json!({"listen": false})),
        )
        .unwrap_or(ListenRequest { listen: false })
        .listen
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
    pub fn get_params_as_map(&self) -> Option<serde_json::Value> {
        if let Ok(params) = serde_json::from_str::<Value>(&self.params_json.clone()) {
            let params_map: Map<String, Value> = params.as_object().unwrap_or(&Map::new()).clone();

            return Some(Value::Object(params_map));
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
        RpcRequest {
            params_json: Self::prepend_ctx(params, &ctx),
            ctx,
            method,
        }
    }

    pub fn get_new_internal_with_appid(
        method: String,
        params: Option<Value>,
        ref_ctx: &CallContext,
    ) -> Self {
        let ctx = CallContext::new(
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            ref_ctx.app_id.clone(),
            1,
            crate::api::gateway::rpc_gateway_api::ApiProtocol::Extn,
            method.clone(),
            None,
            false,
        );
        RpcRequest {
            params_json: Self::prepend_ctx(params, &ctx),
            ctx,
            method,
        }
    }

    pub fn is_rpc_v2(&self) -> bool {
        self.ctx.is_rpc_v2()
    }

    pub fn add_context(&mut self, context: Vec<String>) {
        self.ctx.context.extend(context)
    }
}

#[derive(Debug, Clone, Deserialize)]
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
impl std::fmt::Display for ClientContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "session_id={}, app_id={}, gateway_secure={}",
            self.session_id, self.app_id, self.gateway_secure
        )
    }
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
    #[test]
    fn test_params_as_map() {
        let mut rpc_request = RpcRequest::mock();
        rpc_request.params_json = r#"{"param1": {"key": "value"}}"#.to_string();
        let params = rpc_request.get_params_as_map().unwrap();
        assert!(params.is_object());
        let map = params.as_object().unwrap();
        assert!(map.contains_key("param1"));
        assert_eq!(map.get("param1").unwrap(), &json!({"key": "value"}));
    }
}
