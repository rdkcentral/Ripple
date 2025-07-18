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
use jaq_interpret::{Ctx, FilterT, ParseCtx, RcIter, Val};
use ripple_sdk::api::{
    gateway::rpc_gateway_api::RpcRequest, manifest::extn_manifest::ExtnManifest,
};

use ripple_sdk::{
    chrono::Utc,
    log::{debug, error, info, trace, warn},
    serde_json::Value,
    utils::error::RippleError,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use std::sync::{Mutex, MutexGuard, Once};
use std::{fs, path::Path};

use super::rules_functions::{apply_functions, RulesFunction, RulesImport};

static BASE_PARSE_CTX_INIT: Once = Once::new();
static mut BASE_PARSE_CTX_PTR: Option<Mutex<ParseCtx>> = None;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct RuleSet {
    #[serde(default)]
    pub imports: Vec<String>,
    pub endpoints: HashMap<String, RuleEndpoint>,
    pub rules: HashMap<String, Rule>,
}

impl RuleSet {
    pub fn append(&mut self, rule_set: RuleSet) {
        for import in rule_set.imports {
            if !self.imports.contains(&import) {
                self.imports.push(import);
            }
        }

        self.endpoints.extend(rule_set.endpoints);
        let rules: HashMap<String, Rule> = rule_set
            .rules
            .into_iter()
            .map(|(k, v)| {
                trace!("Loading JQ Rule for {}", k.to_lowercase());
                (k.to_lowercase(), v)
            })
            .collect();
        self.rules.extend(rules);
    }
    pub fn get(&self, key: &str) -> Option<&Rule> {
        self.rules.get(key)
    }
}
#[derive(Debug, Deserialize, Clone, Default)]
pub struct RuleEndpoint {
    pub protocol: RuleEndpointProtocol,
    pub url: String,
    #[serde(default = "default_autostart")]
    pub jsonrpc: bool,
}

impl RuleEndpoint {
    pub fn get_url(&self) -> String {
        if cfg!(feature = "local_dev") {
            if let Ok(host_override) = std::env::var("DEVICE_HOST") {
                if !host_override.is_empty() {
                    return self.url.replace("127.0.0.1", &host_override);
                }
            }
        }
        self.url.clone()
    }
}

fn default_autostart() -> bool {
    true
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(PartialEq))]
pub enum RuleEndpointProtocol {
    #[default]
    Websocket,
    Http,
    Thunder,
    Workflow,
    Extn,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct JsonDataSource {
    // configurable namespace to "stuff" an in individual result payload into
    pub namespace: Option<String>,
    pub method: String,
    pub params: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EventHandler {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub alias: String,
    // Not every rule needs transform
    #[serde(default)]
    pub transform: RuleTransform,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(default)]
    pub event_handler: Option<EventHandler>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<JsonDataSource>>,
}
impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rule {{ alias: {} }}", self.alias)
    }
}
/*
war on dots
*/
#[derive(PartialEq)]
pub enum RuleType {
    Static,
    Provider,
    Endpoint,
}
impl Rule {
    pub fn rule_type(&self) -> RuleType {
        match self.alias.trim().to_ascii_lowercase().as_str() {
            "static" => RuleType::Static,
            "provided" => RuleType::Provider,
            /*
            maps to a sender that can be used to send a message to the endpoint, for instance: Thunder
            */
            _ => RuleType::Endpoint,
        }
    }
    pub fn with_alias(&mut self, alias: String) -> &mut Self {
        self.alias = alias;
        self
    }
    pub fn with_rule_tranformt(&mut self, transform: RuleTransform) -> &mut Self {
        self.transform = transform;
        self
    }
    pub fn with_filter(&mut self, filter: String) -> &mut Self {
        self.filter = Some(filter);
        self
    }
    pub fn with_event_handler(&mut self, event_handler: EventHandler) -> &mut Self {
        self.event_handler = Some(event_handler);
        self
    }
    pub fn with_endpoint(&mut self, endpoint: String) -> &mut Self {
        self.endpoint = Some(endpoint);
        self
    }
    pub fn with_sources(&mut self, sources: Vec<JsonDataSource>) -> &mut Self {
        self.sources = Some(sources);
        self
    }
    pub fn with_source(&mut self, source: JsonDataSource) -> &mut Self {
        if let Some(sources) = &mut self.sources {
            sources.push(source);
        } else {
            self.sources = Some(vec![source]);
        }
        self
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RuleTransform {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rpcv2_event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_decorator_method: Option<String>,
}

impl RuleTransform {
    fn check_and_replace(&self, input: &str, rpc_request: &RpcRequest) -> String {
        trace!(
            "check_and_replace: input: {}, rpc_request: {:?}",
            input,
            rpc_request
        );

        let mut output = input.replace("$context.appId", &rpc_request.ctx.app_id);

        if let Some(event) = &self.event {
            output = output.replace("$event", event);
        }

        output
    }

    pub fn apply_functions(&mut self, imports: &HashMap<String, RulesFunction>) {
        if let Some(transform) = self.request.take() {
            if let Ok(transformed) = apply_functions(&transform, imports) {
                let _ = self.request.insert(transformed);
            }
        }

        if let Some(transform) = self.response.take() {
            if let Ok(transformed) = apply_functions(&transform, imports) {
                let _ = self.response.insert(transformed);
            }
        }
    }

    pub fn apply_variables(&mut self, rpc_request: &RpcRequest) -> &mut Self {
        if let Some(value) = self.request.take() {
            let _ = self
                .request
                .insert(self.check_and_replace(&value, rpc_request));
        }

        if let Some(value) = self.response.take() {
            let _ = self
                .response
                .insert(self.check_and_replace(&value, rpc_request));
        }

        if let Some(value) = self.event.take() {
            let _ = self
                .event
                .insert(self.check_and_replace(&value, rpc_request));
        }

        if let Some(value) = self.rpcv2_event.take() {
            let _ = self
                .rpcv2_event
                .insert(self.check_and_replace(&value, rpc_request));
        }
        self
    }

    pub fn get_transform_data(&self, typ: RuleTransformType) -> Option<String> {
        match typ {
            RuleTransformType::Request => self.request.clone(),
            RuleTransformType::Event(rpc_v2) => {
                if rpc_v2 {
                    self.rpcv2_event.clone()
                } else {
                    self.event.clone()
                }
            }
            RuleTransformType::Response => self.response.clone(),
        }
    }
}

pub enum RuleTransformType {
    Request,
    Response,
    Event(bool),
}

#[derive(Debug, Clone, Default)]
pub struct RuleEngine {
    pub rules: RuleSet,
    pub functions: HashMap<String, RulesFunction>,
}

impl RuleEngine {
    fn build_path(path: &str, default_path: &str) -> String {
        if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("{}{}", default_path, path)
        }
    }

    pub fn load(path: &str) -> Result<RuleEngine, RippleError> {
        let path = Path::new(path);
        if path.exists() {
            let contents = fs::read_to_string(path).unwrap();
            Self::load_from_string_literal(contents)
        } else {
            warn!("path for the rule is invalid {}", path.display());
            Err(RippleError::InvalidInput)
        }
    }
    pub fn load_from_string_literal(contents: String) -> Result<RuleEngine, RippleError> {
        let (_content, rule_set) = Self::load_from_content(contents)?;
        let mut rules_engine = RuleEngine::default();
        rules_engine.rules.append(rule_set);
        Ok(rules_engine.clone())
    }

    pub fn build(extn_manifest: &ExtnManifest) -> Self {
        trace!("building rules engine {:?}", extn_manifest.rules_path);
        let mut engine = RuleEngine::default();
        for path in extn_manifest.rules_path.iter() {
            let path_for_rule = Self::build_path(path, &extn_manifest.default_path);
            debug!("loading rules file {}", path_for_rule);
            if let Some(p) = Path::new(&path_for_rule).to_str() {
                if let Ok(contents) = fs::read_to_string(p) {
                    info!("Rules content {}", contents);
                    info!("loading rules from path {}", path);
                    info!("loading rule {}", path_for_rule);
                    if let Ok((_, rule_set)) = Self::load_from_content(contents) {
                        engine.add_rules(rule_set, &extn_manifest.default_path);
                    } else {
                        warn!("invalid rule found in path {}", path)
                    }
                } else {
                    warn!("path for the rule is invalid {}", path)
                }
            } else {
                warn!("invalid rule path {}", path)
            }
        }
        engine
    }

    fn load_imports(&mut self, imports: &Vec<String>, default_path: &str) {
        for import in imports {
            let path_to_import = Self::build_path(import, default_path);
            match fs::read_to_string(&path_to_import) {
                Ok(import_contents) => {
                    match serde_json::from_str::<RulesImport>(&import_contents) {
                        Ok(import) => {
                            for (function_name, function) in import.functions {
                                // Last loaded import file will overwrite any pre-exsting functions to allow overriding.
                                self.functions
                                    .insert(function_name.clone(), function.clone());
                            }
                        }
                        Err(e) => {
                            error!(
                                "load_imports: Invalid import: path_to_import={}, e={:?}",
                                path_to_import, e
                            )
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "load_imports: Invalid path: path_to_import={}, e={:?}",
                        path_to_import, e
                    );
                }
            }
        }
    }

    pub fn load_from_content(contents: String) -> Result<(String, RuleSet), RippleError> {
        match serde_json::from_str::<RuleSet>(&contents) {
            Ok(manifest) => Ok((contents, manifest)),
            Err(err) => {
                error!("{:?} could not load rule", err);
                Err(RippleError::InvalidInput)
            }
        }
    }
    pub fn add_rules(&mut self, rules: RuleSet, default_path: &str) {
        self.rules.append(rules.clone());
        self.load_imports(&rules.imports, default_path);
    }
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.rules.insert(rule.alias.clone(), rule);
    }

    pub fn has_rule(&self, request: &str) -> bool {
        self.rules.rules.contains_key(&request.to_lowercase())
    }
    fn wildcard_match(rule_name: &str, method: &str) -> bool {
        rule_name.ends_with(".*") && method.starts_with(&rule_name[..rule_name.len() - 1])
    }
    fn find_wildcard_rule(
        rules: &HashMap<String, Rule>,
        method: &str,
    ) -> Result<RuleRetrieved, RuleRetrievalError> {
        let filtered_rules: Vec<&Rule> = rules
            .iter()
            .filter(|(rule_name, _)| Self::wildcard_match(rule_name, method))
            .map(|(_, rule)| rule)
            .collect();

        match filtered_rules.len() {
            1 => Ok(RuleRetrieved::WildcardMatch(filtered_rules[0].clone())),
            0 => Err(RuleRetrievalError::RuleNotFoundAsWildcard),
            _ => Err(RuleRetrievalError::TooManyWildcardMatches),
        }
    }

    fn apply_functions(&self, rule: &mut Rule) {
        rule.transform.apply_functions(&self.functions);
    }

    fn apply_variables(&self, rule: &mut Rule, rpc_request: &RpcRequest) {
        rule.transform.apply_variables(rpc_request);
    }

    pub fn get_rule(&self, rpc_request: &RpcRequest) -> Result<RuleRetrieved, RuleRetrievalError> {
        let method = rpc_request.method.to_lowercase();

        /*
        match directly from method name
         */

        if let Some(mut rule) = self.rules.get(&method).cloned() {
            self.apply_functions(&mut rule);
            self.apply_variables(&mut rule, rpc_request);
            Ok(RuleRetrieved::ExactMatch(rule.to_owned()))
        } else {
            /*
             * match, for example api.v1.* as rule name and api.v1.get as method name
             */
            Self::find_wildcard_rule(&self.rules.rules, &method)
        }
    }

    pub fn get_rule_by_method(&self, method: &str) -> Option<Rule> {
        self.rules.rules.get(&method.to_lowercase()).cloned()
    }
}
#[derive(Debug)]
pub enum RuleRetrieved {
    ExactMatch(Rule),
    WildcardMatch(Rule),
}
impl From<RuleRetrieved> for Rule {
    fn from(rule_retrieved: RuleRetrieved) -> Self {
        match rule_retrieved {
            RuleRetrieved::ExactMatch(rule) => rule,
            RuleRetrieved::WildcardMatch(rule) => rule,
        }
    }
}

#[derive(Debug)]
pub enum RuleRetrievalError {
    RuleNotFound(String),
    RuleNotFoundAsWildcard,
    TooManyWildcardMatches,
}

/// Compiles and executes a JQ filter on a given JSON input value.
///
/// # Arguments
///
/// * `input` - A `serde_json::Value` representing the JSON input to be processed.
/// * `filter` - A string slice that holds the JQ filter to be applied.
/// * `reference` - A string used for logging purposes to identify the rule being processed.
///
/// # Returns
///
/// * `Ok(Value)` - If the filter is successfully compiled and executed, returns the resulting JSON value.
/// * `Err(RippleError)` - If there is an error during the compilation or execution of the filter, returns a `RippleError`.
///
/// # Errors
///
/// This function will return an error if:
/// * The JQ filter contains syntax errors.
/// * There are errors during the compilation of the filter.
/// * The execution of the filter does not produce a valid output.
///
/// # Example
///
/// ```
/// use serde_json::json;
/// use ripple_sdk::utils::error::RippleError;
/// use crate::jq_compile;
///
/// let filter = "if .success then .stbVersion else { code: -32100, message: \"couldn't get version\" } end";
/// let input = json!({
///     "stbVersion": "SCXI11BEI_VBN_24Q2_sprint_20240620140024sdy_FG_GRT",
///     "success": true
/// });
/// let result = jq_compile(input, filter, String::new());
/// assert_eq!(result.unwrap(), json!("SCXI11BEI_VBN_24Q2_sprint_20240620140024sdy_FG_GRT"));
/// ```

// Initializes the base ParseCtx with core and std filters, only once.
fn get_parse_ctx() -> MutexGuard<'static, ParseCtx> {
    BASE_PARSE_CTX_INIT.call_once(|| {
        let mut ctx = ParseCtx::new(Vec::new());
        ctx.insert_natives(jaq_core::core());
        ctx.insert_defs(jaq_std::std());
        unsafe {
            BASE_PARSE_CTX_PTR = Some(Mutex::new(ctx));
        }
    });
    unsafe {
        BASE_PARSE_CTX_PTR
            .as_ref()
            .expect("BASE_PARSE_CTX_PTR not initialized")
            .lock()
            .expect("Failed to lock BASE_PARSE_CTX_PTR")
    }
}

pub fn jq_compile(input: Value, filter: &str, reference: String) -> Result<Value, RippleError> {
    info!(
        "Jq rule {}  input {:?}, reference {}",
        filter, input, reference
    );
    let start = Utc::now().timestamp_millis();
    // start out only from core filters,
    // which do not include filters in the standard library
    // such as `map`, `select` etc.

    // Parse the filter
    let (f, errs) = jaq_parse::parse(filter, jaq_parse::main());
    if !errs.is_empty() {
        error!("Error in rule {:?}: {:?}", reference, errs);
        return Err(RippleError::RuleError);
    }
    // Lock and use the shared ParseCtx
    let mut defs = get_parse_ctx();
    // compile the filter in the context of the given definitions
    let f = defs.compile(f.unwrap());
    if !defs.errs.is_empty() {
        error!("Error in rule {}", reference);
        for (err, _) in &defs.errs {
            error!("reference={} {}", reference, err);
        }
        defs.errs.clear(); // Clear errors before returning
        return Err(RippleError::RuleError);
    }
    let inputs = RcIter::new(core::iter::empty());
    // iterator over the output values
    let mut out = f.run((Ctx::new([], &inputs), Val::from(input)));
    if let Some(Ok(v)) = out.next() {
        info!(
            "Ripple Gateway Rule Processing Time: {},{}",
            reference,
            Utc::now().timestamp_millis() - start
        );
        return Ok(Value::from(v));
    }

    Err(RippleError::ParseError)
}

pub fn compose_json_values(values: Vec<Value>) -> Value {
    if values.len() == 1 {
        return values[0].clone();
    }
    debug!("Composing values {:?}", values);

    let mut composition_filter = ".[0]".to_string();
    for v in 1..values.len() {
        composition_filter = format!("{} * .[{}]", composition_filter, v);
    }
    match jq_compile(Value::Array(values), &composition_filter, String::new()) {
        Ok(composed_value) => composed_value,
        Err(err) => {
            error!("Failed to compose JSON values with error: {:?}", err);
            Value::Null // Return a default value on failure
        }
    }
}
pub fn make_name_json_safe(name: &str) -> String {
    name.replace([' ', '.', ','], "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ripple_sdk::api::gateway::rpc_gateway_api::RpcRequest;
    use ripple_sdk::serde_json::json;

    #[test]
    fn test_jq_compile() {
        let filter = "if .success then ( .stbVersion | split(\"_\")[0] ) else { code: -32100, message: \"couldn't get version\" } end";
        let input = json!({
            "stbVersion":"SCXI11BEI_VBN_24Q2_sprint_20240620140024sdy_FG_GRT",
            "receiverVersion":"7.2.0.0",
            "stbTimestamp":"Thu 20 Jun 2024 14:00:24 UTC",
            "success":true
        });
        let resp = jq_compile(input, filter, String::new());
        assert_eq!(resp.unwrap(), "SCXI11BEI".to_string());

        let filter = "{ namespace: \"refui\", scope: .scope, key: .key, value: .value }";
        let input = json!({
            "key": "key3",
            "scope": "account",
            "value": "value2"
        });
        let resp = jq_compile(input, filter, String::new());
        let expected = json!({
           "namespace": "refui",
           "key": "key3",
           "scope": "account",
           "value": "value2"
        });
        assert_eq!(resp.unwrap(), expected);

        let filter = "if .success and ( .supportedHDCPVersion | contains(\"2.2\")) then {\"hdcp2.2\": true} elif .success and ( .supportedHDCPVersion | contains(\"1.4\")) then {\"hdcp1.4\": true}  else {\"code\": -32100, \"message\": \"couldn't get version\"} end";
        let input = json!({
            "supportedHDCPVersion":"2.2",
            "isHDCPSupported":true,
            "success":true
        });
        let resp = jq_compile(input, filter, String::new());
        let expected = json!({
           "hdcp2.2": true
        });
        assert_eq!(resp.unwrap(), expected);

        let filter = "if .success then (.value | fromjson | .value) else { \"code\": -32100, \"message\": \"couldn't get language\" } end";
        let input = json!({
           "value": "{\"update_time\":\"2024-07-26T23:39:57.831726080Z\",\"value\":\"EN\"}",
           "success":true
        });
        let resp = jq_compile(input, filter, String::new());
        assert_eq!(resp.unwrap(), "EN".to_string());
    }
    #[test]
    fn test_composed_jq_compile() {
        let a = json!({"asome": "avalue"});
        let b = json!({"bsome": "bvalue"});
        let c = json!({"csome": {"cvalue" : "nested"}});
        let vals = vec![a, b, c];
        let mut composition_filter = ".[0]".to_string();
        for v in 1..vals.len() {
            composition_filter = format!("{} * .[{}]", composition_filter, v);
        }

        assert!(jq_compile(
            jq_compile(
                Value::Array(vals.clone()),
                &composition_filter,
                String::new()
            )
            .unwrap(),
            ".asome",
            String::new()
        )
        .unwrap()
        .as_str()
        .unwrap()
        .contains("avalue"));
        assert!(jq_compile(
            jq_compile(
                Value::Array(vals.clone()),
                &composition_filter,
                String::new()
            )
            .unwrap(),
            ".bsome",
            String::new()
        )
        .unwrap()
        .as_str()
        .unwrap()
        .contains("bvalue"));
        assert!(jq_compile(
            jq_compile(
                Value::Array(vals.clone()),
                &composition_filter,
                String::new()
            )
            .unwrap(),
            ".csome.cvalue",
            String::new()
        )
        .unwrap()
        .as_str()
        .unwrap()
        .contains("nested"));
    }
    use ripple_sdk::api::gateway::rpc_gateway_api::CallContext;

    #[test]
    fn test_get_rule_exact_match() {
        let mut rule_set = RuleSet::default();
        let rule = Rule {
            alias: "test_rule".to_string(),
            ..Default::default()
        };
        rule_set
            .rules
            .insert("test.method".to_string(), rule.clone());

        let rule_engine = RuleEngine {
            rules: rule_set,
            functions: HashMap::default(),
        };

        let rpc_request = RpcRequest {
            method: "test.method".to_string(),
            ctx: CallContext {
                app_id: "test_app".to_string(),
                method: "test.method".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = rule_engine.get_rule(&rpc_request);
        match result {
            Ok(RuleRetrieved::ExactMatch(retrieved_rule)) => {
                assert_eq!(retrieved_rule.alias, rule.alias);
            }
            _ => panic!("Expected exact match, but got {:?}", result),
        }
    }

    #[test]
    fn test_get_rule_wildcard_match() {
        let mut rule_set = RuleSet::default();
        let rule = Rule {
            alias: "wildcard_rule".to_string(),
            ..Default::default()
        };
        rule_set.rules.insert("api.v1.*".to_string(), rule.clone());

        let rule_engine = RuleEngine {
            rules: rule_set,
            functions: HashMap::default(),
        };

        let rpc_request = RpcRequest {
            method: "api.v1.get".to_string(),
            ctx: CallContext {
                app_id: "test_app".to_string(),
                method: "api.v1.get".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = rule_engine.get_rule(&rpc_request);
        match result {
            Ok(RuleRetrieved::WildcardMatch(retrieved_rule)) => {
                assert_eq!(retrieved_rule.alias, rule.alias);
            }
            _ => panic!("Expected wildcard match, but got {:?}", result),
        }
    }

    #[test]
    fn test_get_rule_no_match() {
        let rule_set = RuleSet::default();
        let rule_engine = RuleEngine {
            rules: rule_set,
            functions: HashMap::default(),
        };

        let rpc_request = RpcRequest {
            method: "nonexistent.method".to_string(),
            ctx: CallContext {
                app_id: "test_app".to_string(),
                method: "nonexistent.method".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let result = rule_engine.get_rule(&rpc_request);
        assert!(matches!(
            result,
            Err(RuleRetrievalError::RuleNotFoundAsWildcard)
        ));
    }
}
