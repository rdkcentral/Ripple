use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};

use crate::{
    extn::extn_client_message::{ExtnPayload, ExtnPayloadProvider, ExtnRequest},
    framework::ripple_contract::{MainContract, RippleContract},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CallContext {
    pub session_id: String,
    pub request_id: String,
    pub app_id: String,
    pub call_id: u64,
    pub protocol: ApiProtocol,
    pub method: String,
}

impl CallContext {
    pub fn new(
        session_id: String,
        request_id: String,
        app_id: String,
        call_id: u64,
        protocol: ApiProtocol,
        method: String,
    ) -> CallContext {
        CallContext {
            session_id,
            request_id,
            app_id,
            call_id,
            protocol,
            method,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ApiProtocol {
    Bridge,
    Extn,
    JsonRpc,
}

#[derive(Clone, Debug)]
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

    /// Converts this jsonrpc formatted message into bridge message
    /// Handles the different protocols and the conversion between them.
    /// If the protocol is bridge then it is first converted to a bridgeApiResponse message
    /// If the protocol is jsonrpc, then it is simply wrapped in a bridge object wrapper
    /// For bridge protocol, handle jsonrpc errors when converting to the bridge protocol.
    /// bridge seems to handle errors strangely when JSON is returned, adding a prefix to the error
    /// allowed the error to be seen in the bridge reference app.
    pub fn as_bridge_msg(&self) -> String {
        match self.protocol {
            ApiProtocol::Bridge => {
                let resp: JsonRpcApiResponse = serde_json::from_str(&self.jsonrpc_msg).unwrap();
                let bridge_resp = BridgeApiResponse {
                    pid: resp.id,
                    success: match resp.error {
                        Some(_) => false,
                        None => true,
                    },
                    json: match resp.error {
                        // If the error has data then use data as the bridge payload in the error repsonse
                        Some(err) => match err.get("data") {
                            Some(data) => Some(data.clone()),
                            None => Some(err),
                        },
                        None => resp.result,
                    },
                };

                serde_json::to_string(&bridge_resp).unwrap()
            }
            ApiProtocol::JsonRpc => format!("{{\"json\":{}}}", self.jsonrpc_msg),
            ApiProtocol::Extn => self.jsonrpc_msg.clone(),
        }
    }
}

#[derive(Deserialize)]
struct ApiBaseRequest {
    jsonrpc: Option<String>,
}

impl ApiBaseRequest {
    fn is_jsonrpc(&self) -> bool {
        match self.jsonrpc {
            Some(_) => true,
            None => false,
        }
    }
}

#[derive(Deserialize)]
pub struct BridgeApiRequest {
    pub pid: Option<u64>,
    pub action: String,
    pub args: Option<Value>,
}

#[derive(Serialize, Deserialize)]
pub struct BridgeApiResponse {
    pub pid: u64,
    pub success: bool,
    pub json: Option<Value>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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
        RippleContract::Main(MainContract::Rpc)
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
            ps.push(json!(params.clone()));
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
        match base.is_jsonrpc() {
            true => {
                let jsonrpc_req_res = serde_json::from_value(parsed);
                if jsonrpc_req_res.is_err() {
                    return Err(RequestParseError {});
                }
                let jsonrpc_req: JsonRpcApiRequest = jsonrpc_req_res.unwrap();
                let id = match jsonrpc_req.id {
                    Some(n) => n,
                    None => 0,
                };
                let ctx = CallContext::new(
                    session_id,
                    request_id,
                    app_id,
                    id,
                    ApiProtocol::JsonRpc,
                    jsonrpc_req.method.clone(),
                );
                let ps = RpcRequest::prepend_ctx(jsonrpc_req.params, &ctx);
                Ok(RpcRequest::new(jsonrpc_req.method, ps, ctx))
            }
            false => {
                let badger_req_resp = serde_json::from_value(parsed);
                if badger_req_resp.is_err() {
                    return Err(RequestParseError {});
                }
                let badger_req: BridgeApiRequest = badger_req_resp.unwrap();
                let id = match badger_req.pid {
                    Some(n) => n,
                    None => 0,
                };
                let method = format!("bridge.{}", badger_req.action);
                let ctx = CallContext::new(
                    session_id,
                    request_id,
                    app_id,
                    id,
                    ApiProtocol::Bridge,
                    method,
                );
                let method = format!("bridge.{}", badger_req.action);
                let ps = RpcRequest::prepend_ctx(badger_req.args, &ctx);
                Ok(RpcRequest::new(method, ps, ctx))
            }
        }
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
}

#[derive(Debug)]
pub enum PermissionCommand {
    GateRequest {
        req: RpcRequest,
        route_tx: oneshot::Sender<bool>,
        session_tx: mpsc::Sender<ApiMessage>,
    },
}
