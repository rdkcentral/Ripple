use std::collections::HashMap;

use crate::api::gateway::rpc_gateway_api::{
    CallContext, ClientContext, JsonRpcApiResponse, RpcRequest,
};

/*

Abstractions around ease of use contextual logging
*/
pub trait ContextAsJson {
    fn as_json(&self) -> serde_json::Value;
}
#[derive(serde::Serialize, Clone)]
pub struct LogSignal<T>
where
    T: std::fmt::Display + ContextAsJson,
{
    name: String,
    message: String,
    diagnostic_context: HashMap<String, String>,
    context: T,
}
impl<T: std::fmt::Display + ContextAsJson> std::fmt::Display for LogSignal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "message={}, diagnostic_context={}, call_context={}",
            self.message,
            diagnostic_context_to_string(&self.diagnostic_context),
            self.context
        )
    }
}

fn map_to_jsonmap(map: HashMap<String, String>) -> serde_json::Map<String, serde_json::Value> {
    let mut json_map = serde_json::Map::new();
    for (key, value) in map {
        json_map.insert(key, serde_json::Value::String(value));
    }
    json_map
}
fn diagnostic_context_to_string(diagnostic_context: &HashMap<String, String>) -> String {
    let mut diagnostic_context_string = String::new();
    for (key, value) in diagnostic_context {
        diagnostic_context_string.push_str(&format!("{}:{} ", key, value));
    }
    diagnostic_context_string
}

impl ContextAsJson for CallContext {
    fn as_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "session_id".to_string(),
            serde_json::Value::String(self.session_id.clone()),
        );
        map.insert(
            "request_id".to_string(),
            serde_json::Value::String(self.request_id.clone()),
        );
        map.insert(
            "app_id".to_string(),
            serde_json::Value::String(self.app_id.clone()),
        );
        map.insert(
            "call_id".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.call_id)),
        );
        map.insert(
            "method".to_string(),
            serde_json::Value::String(self.method.clone()),
        );
        map.insert(
            "cid".to_string(),
            serde_json::Value::String(self.cid.clone().unwrap_or_default()),
        );
        map.insert(
            "gateway_secure".to_string(),
            serde_json::Value::Bool(self.gateway_secure),
        );
        serde_json::Value::Object(map)
    }
}
impl ContextAsJson for ClientContext {
    fn as_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "session_id".to_string(),
            serde_json::Value::String(self.session_id.clone()),
        );
        map.insert(
            "app_id".to_string(),
            serde_json::Value::String(self.app_id.clone()),
        );
        serde_json::Value::Object(map)
    }
}
impl ContextAsJson for JsonRpcApiResponse {
    fn as_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "session_id".to_string(),
            match self.id {
                Some(id) => serde_json::Value::Number(id.into()),
                None => serde_json::Value::Null,
            },
        );

        map.insert(
            "jsonrpc".to_string(),
            serde_json::Value::String(self.jsonrpc.clone()),
        );
        map.insert(
            "result".to_string(),
            self.result.clone().unwrap_or_default(),
        );
        map.insert(
            "method".to_string(),
            serde_json::Value::String(self.method.clone().unwrap_or_default()),
        );
        map.insert("error".to_string(), self.error.clone().unwrap_or_default());

        serde_json::Value::Object(map)
    }
}
impl ContextAsJson for RpcRequest {
    fn as_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "method".to_string(),
            serde_json::Value::String(self.method.clone()),
        );
        map.insert(
            "params".to_string(),
            serde_json::Value::String(self.params_json.clone()),
        );
        map.insert("call_context".to_string(), self.ctx.as_json());
        serde_json::Value::Object(map)
    }
}
impl std::fmt::Display for RpcRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rpc_request={}", self.method)
    }
}

impl<T> From<&LogSignal<T>> for serde_json::Value
where
    T: std::fmt::Display + ContextAsJson,
{
    fn from(signal: &LogSignal<T>) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "message".to_string(),
            serde_json::Value::String(signal.message.clone()),
        );
        map.insert(
            "name".to_string(),
            serde_json::Value::String(signal.name.clone()),
        );

        map.insert(
            "diagnostic_context".to_string(),
            serde_json::Value::Object(map_to_jsonmap(signal.diagnostic_context.clone())),
        );
        map.insert("call_context".to_string(), signal.context.as_json());
        serde_json::json!({"log_signal": serde_json::Value::Object(map) })
    }
}
impl std::fmt::Display for JsonRpcApiResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session_id={}", self.id.unwrap_or_default())
    }
}

impl<T: std::fmt::Display + ContextAsJson> LogSignal<T> {
    pub fn new(name: String, message: String, context: T) -> Self
    where
        T: ContextAsJson,
    {
        LogSignal {
            name,
            message,
            diagnostic_context: HashMap::new(),
            context,
        }
    }
    pub fn emit_debug(&self) {
        log::debug!("{}", serde_json::Value::from(self));
    }
    pub fn emit_error(&self) {
        log::error!("{}", serde_json::Value::from(self));
    }

    pub fn with_diagnostic_context(mut self, diagnostic_context: HashMap<String, String>) -> Self {
        self.diagnostic_context = diagnostic_context;
        self
    }
    pub fn with_diagnostic_context_item(mut self, key: &str, value: &str) -> Self {
        self.diagnostic_context
            .insert(key.to_string(), value.to_string());
        self
    }
}
/*write unit tests for this file */
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::gateway::rpc_gateway_api::CallContext;
    use crate::Mockable;
    use std::collections::HashMap;
    #[test]
    fn test_log_signal_json_output() {
        let mut diagnostic_context = HashMap::new();
        diagnostic_context.insert("key".to_string(), "value".to_string());
        let call_context = CallContext::mock();
        let log_signal = LogSignal::new("tester".to_string(), "message".to_string(), call_context)
            .with_diagnostic_context(diagnostic_context);
        let json = serde_json::to_string(&log_signal).unwrap();
        assert_eq!(json, "{\"name\":\"tester\",\"message\":\"message\",\"diagnostic_context\":{\"key\":\"value\"},\"context\":{\"session_id\":\"session_id\",\"request_id\":\"1\",\"app_id\":\"some_app_id\",\"call_id\":1,\"protocol\":\"JsonRpc\",\"method\":\"module.method\",\"cid\":\"cid\",\"gateway_secure\":true}}");
    }
    #[test]
    fn test_log_signal_text_output() {
        let mut diagnostic_context = HashMap::new();
        diagnostic_context.insert("key".to_string(), "value".to_string());
        let call_context = CallContext::mock();
        let log_signal = LogSignal::new("tester".to_string(), "message".to_string(), call_context)
            .with_diagnostic_context(diagnostic_context);
        let text = format!("{}", log_signal);
        assert_eq!(text, "message=message, diagnostic_context=key:value , call_context=session_id=session_id, request_id=1, app_id=some_app_id, call_id=1, protocol=JsonRpc, method=module.method, cid=cid");
    }
}
