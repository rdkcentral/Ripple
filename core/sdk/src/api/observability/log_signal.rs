use std::collections::HashMap;

use crate::api::gateway::rpc_gateway_api::CallContext;

/*

Abstractions around ease of use contextual logging
*/
#[derive(serde::Serialize)]
pub struct LogSignal {
    message: String,
    diagnostic_context: HashMap<String, String>,
    call_context: CallContext,
}
impl std::fmt::Display for LogSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "message={}, diagnostic_context={}, call_context={}",
            self.message,
            diagnostic_context_to_string(&self.diagnostic_context),
            self.call_context
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

impl From<LogSignal> for serde_json::Value {
    fn from(signal: LogSignal) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "message".to_string(),
            serde_json::Value::String(signal.message),
        );
        map.insert(
            "diagnostic_context".to_string(),
            serde_json::Value::Object(map_to_jsonmap(signal.diagnostic_context.into())),
        );
        map.insert("call_context".to_string(), signal.call_context.into());
        serde_json::Value::Object(map)
    }
}

impl LogSignal {
    pub fn new(message: String, call_context: CallContext) -> Self {
        LogSignal {
            message,
            diagnostic_context: HashMap::new(),
            call_context,
        }
    }

    pub fn with_diagnostic_context(mut self, diagnostic_context: HashMap<String, String>) -> Self {
        self.diagnostic_context = diagnostic_context;
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
        let log_signal = LogSignal::new("message".to_string(), call_context)
            .with_diagnostic_context(diagnostic_context);
        let json = serde_json::to_string(&log_signal).unwrap();
        assert_eq!(json, "{\"message\":\"message\",\"diagnostic_context\":{\"key\":\"value\"},\"call_context\":{\"session_id\":\"session_id\",\"request_id\":\"1\",\"app_id\":\"some_app_id\",\"call_id\":1,\"protocol\":\"JsonRpc\",\"method\":\"module.method\",\"cid\":\"cid\",\"gateway_secure\":true}}");
    }
    #[test]
    fn test_log_signal_text_output() {
        let mut diagnostic_context = HashMap::new();
        diagnostic_context.insert("key".to_string(), "value".to_string());
        let call_context = CallContext::mock();
        let log_signal = LogSignal::new("message".to_string(), call_context)
            .with_diagnostic_context(diagnostic_context);
        let text = format!("{}", log_signal);
        assert_eq!(text, "message=message, diagnostic_context=key:value , call_context=session_id=session_id, request_id=1, app_id=some_app_id, call_id=1, protocol=JsonRpc, method=module.method, cid=cid");
    }
}
