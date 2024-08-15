use std::{collections::HashMap, fs};

use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub extern crate jsonschema;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FireboltOpenRpc {
    pub apis: HashMap<String, FireboltOpenRpcSpec>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ValidationError {
    SpecVersionNotFound,
    MethodNotFound,
    InvalidSchema,
}

impl FireboltOpenRpc {
    pub fn expect_from_file_path(path: &str) -> FireboltOpenRpc {
        let data = fs::read_to_string(path).expect("Unable to read file");
        serde_json::from_str(&data).expect("JSON does not have correct format.")
    }

    pub fn get_method_by_name(&self, name: &str) -> Option<RpcMethod> {
        for spec in self.apis.values() {
            for m in &spec.methods {
                if m.name.to_ascii_lowercase() == name.to_ascii_lowercase() {
                    return Some(m.clone());
                }
            }
        }
        None
    }

    // <pca>
    pub fn get_result_properties_schema_by_name(&self, name: &str) -> Option<Map<String, Value>> {
        if let Some(method) = self.get_method_by_name(name) {
            if let Some(result_schema_map) = method.result.schema.as_object() {
                println!(
                    "*** _DEBUG: get_result_properties_schema_by_name: result_schema_map={:?}",
                    result_schema_map
                );
                if let Some(result_schema_value) = result_schema_map.get("$ref") {
                    if let Some(result_schema_string) = result_schema_value.as_str() {
                        println!(
                            "*** _DEBUG: get_result_properties_schema_by_name: result_schema_string={}",
                            result_schema_string
                        );
                        let result_type_string = result_schema_string.split("/").last().unwrap();
                        println!(
                            "*** _DEBUG: get_result_properties_schema_by_name: result_type_string={}",
                            result_type_string
                        );
                        for spec in self.apis.values() {
                            if let Value::Object(components) = &spec.components {
                                if let Some(schemas_value) = components.get("schemas") {
                                    println!(
                                        "*** _DEBUG: get_result_properties_schema_by_name: schemas_value={:?}",
                                        schemas_value
                                    );
                                    if let Value::Object(schemas_map) = schemas_value {
                                        if let Some(result_type_value) =
                                            schemas_map.get(result_type_string)
                                        {
                                            println!(
                                                "*** _DEBUG: get_result_properties_schema_by_name: result_type_value={:?}",
                                                result_type_value
                                            );
                                            if let Some(result_type_map) =
                                                result_type_value.as_object()
                                            {
                                                println!(
                                                    "*** _DEBUG: get_result_properties_schema_by_name: result_type_map={:?}",
                                                    result_type_map
                                                );
                                                if let Some(result_properties) =
                                                    result_type_map.get("properties")
                                                {
                                                    println!(
                                                        "*** _DEBUG: get_result_properties_schema_by_name: result_properties={:?}",
                                                        result_properties
                                                    );
                                                    if let Value::Object(result_properties_map) =
                                                        result_properties
                                                    {
                                                        println!(
                                                            "*** _DEBUG: get_result_properties_schema_by_name: result_properties_map={:?}",
                                                            result_properties_map
                                                        );
                                                        return Some(result_properties_map.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
    // </pca>

    pub fn params_validator(
        &self,
        version: String,
        method: &str,
    ) -> Result<JSONSchema, ValidationError> {
        if let Some(spec) = self.apis.get(&version) {
            let open_rpc_spec: OpenRpcSpec = spec.clone().into();
            open_rpc_spec.params_validator(method)
        } else {
            Err(ValidationError::SpecVersionNotFound)
        }
    }

    pub fn result_validator(
        &self,
        version: String,
        method: String,
    ) -> Result<JSONSchema, ValidationError> {
        if let Some(spec) = self.apis.get(&version) {
            let open_rpc_spec: OpenRpcSpec = spec.clone().into();
            open_rpc_spec.result_validator(method)
        } else {
            Err(ValidationError::SpecVersionNotFound)
        }
    }
}

pub struct JsonRpcRequest {
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FireboltOpenRpcSpec {
    pub methods: Vec<RpcMethod>,
    pub components: Value,
    #[serde(rename = "x-schemas")]
    pub x_schemas: Value,
}

impl From<FireboltOpenRpcSpec> for OpenRpcSpec {
    fn from(value: FireboltOpenRpcSpec) -> Self {
        let mut additional_schemas = HashMap::default();
        additional_schemas.insert(String::from("components"), value.components);
        additional_schemas.insert(String::from("x-schemas"), value.x_schemas);
        OpenRpcSpec {
            methods: value.methods,
            additional_schemas,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenRpcSpec {
    pub methods: Vec<RpcMethod>,
    pub additional_schemas: HashMap<String, Value>,
}

impl OpenRpcSpec {
    pub fn params_validator(&self, method: &str) -> Result<JSONSchema, ValidationError> {
        let method = self.methods.iter().find(|m| m.name == method);
        if let Some(m) = method {
            m.params_validator(self.additional_schemas.clone())
        } else {
            Err(ValidationError::MethodNotFound)
        }
    }

    pub fn result_validator(&self, method: String) -> Result<JSONSchema, ValidationError> {
        let rpc_method = self.methods.iter().find(|m| m.name == method);
        if let Some(m) = rpc_method {
            m.result_validator(self.additional_schemas.clone())
        } else {
            Err(ValidationError::MethodNotFound)
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcMethod {
    pub name: String,
    pub params: Vec<RpcParam>,
    pub result: RpcResult,
    pub examples: Vec<MethodExample>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JsonSchemaObject {
    pub r#type: String,
    pub properties: HashMap<String, Value>,
    pub required: Vec<String>,
}

impl JsonSchemaObject {
    pub fn new(properties: HashMap<String, Value>, required: Vec<String>) -> JsonSchemaObject {
        JsonSchemaObject {
            r#type: String::from("object"),
            properties,
            required,
        }
    }
}

impl RpcMethod {
    ///
    /// Converts an openrpc schema into a jsonschema
    /// for the request params on this method
    pub fn params_schema(&self) -> JsonSchemaObject {
        let mut props = HashMap::default();
        let mut required = Vec::new();
        for param in &self.params {
            props.insert(param.name.clone(), param.schema.clone());
            if param.required {
                required.push(param.name.clone());
            }
        }

        JsonSchemaObject::new(props, required)
    }

    ///
    /// Takes the given base schema and extends it with
    /// additional schemas so that $refs in the base schema
    /// can be resolved
    fn complete_schema(
        mut base: Value,
        additional_schemas: HashMap<String, Value>,
    ) -> Result<JSONSchema, ValidationError> {
        if !base.is_object() {
            return Err(ValidationError::InvalidSchema);
        }
        let obj = base.as_object_mut().unwrap();
        obj.extend(additional_schemas);

        let json = json!(obj);
        let compiled_res = JSONSchema::compile(&json);
        if compiled_res.is_err() {
            return Err(ValidationError::InvalidSchema);
        }
        Ok(compiled_res.unwrap())
    }

    pub fn params_validator(
        &self,
        additional_schemas: HashMap<String, Value>,
    ) -> Result<JSONSchema, ValidationError> {
        let schema = json!(self.params_schema());
        RpcMethod::complete_schema(schema, additional_schemas)
    }

    pub fn result_validator(
        &self,
        additional_schemas: HashMap<String, Value>,
    ) -> Result<JSONSchema, ValidationError> {
        RpcMethod::complete_schema(self.result.schema.clone(), additional_schemas)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcParam {
    name: String,
    #[serde(default)]
    required: bool,
    schema: Value,
    summary: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcResult {
    name: String,
    // <pca>
    //schema: Value,
    pub schema: Value,
    // </pca>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MethodExample {
    name: String,
    params: Vec<NameValue>,
    result: NameValue,
}

impl MethodExample {
    pub fn to_json(&self) -> Value {
        let mut map: HashMap<String, Value> = HashMap::default();
        for p in &self.params {
            map.insert(p.name.clone(), p.value.clone());
        }
        json!(map)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NameValue {
    name: String,
    value: Value,
}

#[cfg(test)]
pub mod tests {

    use jsonschema::ErrorIterator;

    use crate::OpenRpcSpec;

    use super::{FireboltOpenRpc, JsonRpcRequest};

    const RPC_FILE: &str = "./src/test/firebolt-open-rpc.json";
    const SECURE_STORAGE_VALID: &str = r#"
        {
            "scope": "account",
            "key": "a"
        }
    "#;

    pub fn assert_valid(method_name: &str, res: Result<(), ErrorIterator>) {
        let err_msg = res.err().map(|e| {
            let mut error_lines = vec![];
            for error in e {
                error_lines.push(format!("Error at {} is {}", error, error.instance_path));
            }
            format!(
                "{} had an invalid example, error is {}",
                method_name,
                error_lines.join("\n")
            )
        });
        assert!(err_msg.is_none(), "{}", err_msg.unwrap_or(String::from("")));
    }

    #[test]
    pub fn test_all_examples() {
        let rpc = FireboltOpenRpc::expect_from_file_path(RPC_FILE);
        for spec in rpc.apis.values() {
            let open_rpc_spec: OpenRpcSpec = spec.clone().into();
            for method in &spec.methods {
                for ex in &method.examples {
                    let example_json = ex.to_json();

                    let request = &JsonRpcRequest {
                        method: method.name.clone(),
                        params: example_json,
                    };
                    // validate params
                    let validator = open_rpc_spec
                        .params_validator(&request.method.clone())
                        .unwrap();
                    let res = validator.validate(&request.params);
                    assert_valid(&method.name, res);

                    // validate result
                    let validator = open_rpc_spec.result_validator(method.name.clone()).unwrap();
                    let res = validator.validate(&ex.result.value);
                    assert_valid(&method.name, res);
                }
            }
        }
    }

    pub fn test_valid_and_invalid(method: &str, valid_params: &str, invalid_params: &str) {
        let rpc = FireboltOpenRpc::expect_from_file_path(RPC_FILE);
        let valid_req = JsonRpcRequest {
            method: method.into(),
            params: serde_json::from_str(valid_params).unwrap(),
        };
        let validator = rpc
            .params_validator("1".into(), &valid_req.method.clone())
            .unwrap();
        assert_valid(method, validator.validate(&valid_req.params));

        let invalid_req = JsonRpcRequest {
            method: method.into(),
            params: serde_json::from_str(invalid_params).unwrap(),
        };
        let validator = rpc
            .params_validator("1".into(), &invalid_req.method.clone())
            .unwrap();
        assert!(
            validator.validate(&invalid_req.params).is_err(),
            "{} should have failed validation with {}",
            method,
            invalid_params
        );
    }

    #[test]
    pub fn test_bool_instead_of_string() {
        test_valid_and_invalid(
            "SecureStorage.get",
            SECURE_STORAGE_VALID,
            r#"
            {
                "scope": "account",
                "key": true
            }
            "#,
        );
    }

    #[test]
    pub fn test_num_instead_of_string() {
        test_valid_and_invalid(
            "SecureStorage.get",
            SECURE_STORAGE_VALID,
            r#"
            {
                "scope": "account",
                "key": 1
            }
            "#,
        );
    }

    #[test]
    pub fn test_obj_instead_of_string() {
        test_valid_and_invalid(
            "SecureStorage.get",
            SECURE_STORAGE_VALID,
            r#"
            {
                "scope": "account",
                "key": {}
            }
            "#,
        );
    }

    #[test]
    pub fn test_string_not_in_enum() {
        test_valid_and_invalid(
            "SecureStorage.get",
            SECURE_STORAGE_VALID,
            r#"
            {
                "scope": "bogus",
                "key": "a"
            }
            "#,
        );
    }

    #[test]
    pub fn test_missing_required_field() {
        test_valid_and_invalid(
            "SecureStorage.get",
            SECURE_STORAGE_VALID,
            r#"
            {
                "scope": "account"
            }
            "#,
        );
    }

    #[test]
    pub fn test_value_outside_of_min_range() {
        test_valid_and_invalid(
            "Metrics.mediaProgress",
            r#"
            {
                "entityId": "abc",
                "progress": 0
            }
            "#,
            r#"
            {
                "entityId": "abc",
                "progress": -1
            }
            "#,
        );
    }

    #[test]
    pub fn test_value_outside_of_max_range() {
        test_valid_and_invalid(
            "Metrics.mediaProgress",
            r#"
            {
                "entityId": "abc",
                "progress": 86400
            }
            "#,
            r#"
            {
                "entityId": "abc",
                "progress": 86401
            }
            "#,
        );
    }

    #[test]
    pub fn test_unmatched_regex() {
        test_valid_and_invalid(
            "ClosedCaptions.setPreferredLanguages",
            r#"
            {
                "value": ["spa"]
            }
            "#,
            r#"
            {
                "value": ["sp"]
            }
            "#,
        );
    }

    #[test]
    pub fn test_array_wrong_type() {
        test_valid_and_invalid(
            "Discovery.signIn",
            r#"
                {
                    "entitlements": [
                        {
                            "entitlementId": "abc"
                        }
                    ]
                }
            "#,
            r#"
                {
                    "entitlements": ["abc"]
                }
            "#,
        );
    }

    #[test]
    pub fn test_object_param_wrong_field_type() {
        test_valid_and_invalid(
            "Advertising.advertisingId",
            r#"
                {
                    "options": {
                        "scope": {
                            "id": "paidPlacement",
                            "type": "browse"
                        }
                    }
                }
            "#,
            r#"
                {
                    "options": {
                        "scope": {
                            "id": 5,
                            "type": "browse"
                        }
                    }
                }
            "#,
        );
    }
}
