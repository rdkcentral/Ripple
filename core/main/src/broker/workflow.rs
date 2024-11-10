struct JsonDataSource {
    method: String,
    params: Option<serde_json::Value>,
    namespace: String,
}

impl Default for JsonDataSource {
    fn default() -> Self {
        JsonDataSource {
            method: "default_method".to_string(),
            params: None,
            namespace: "default_namespace".to_string(),
        }
    }
}
