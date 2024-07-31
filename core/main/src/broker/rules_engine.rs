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
use ripple_sdk::log::trace;
use ripple_sdk::{
    chrono::Utc,
    log::{debug, error, info, warn},
    serde_json::Value,
    utils::error::RippleError,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::{fs, path::Path};

#[derive(Debug, Deserialize, Default, Clone)]
pub struct RuleSet {
    pub endpoints: HashMap<String, RuleEndpoint>,
    pub rules: HashMap<String, Rule>,
}

impl RuleSet {
    pub fn append(&mut self, rule_set: RuleSet) {
        self.endpoints.extend(rule_set.endpoints);
        self.rules.extend(rule_set.rules);
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct RuleEndpoint {
    pub protocol: RuleEndpointProtocol,
    pub url: String,
    #[serde(default = "default_autostart")]
    pub jsonrpc: bool,
}

fn default_autostart() -> bool {
    true
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(PartialEq))]
pub enum RuleEndpointProtocol {
    Websocket,
    Http,
    Thunder,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub alias: String,
    // Not every rule needs transform
    #[serde(default)]
    pub transform: RuleTransform,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RuleTransform {
    pub request: Option<String>,
    pub response: Option<String>,
    pub event: Option<String>,
}

impl RuleTransform {
    pub fn apply_context(&mut self, rpc_request: &RpcRequest) {
        if let Some(value) = self.request.take() {
            debug!("Check if value contains {}", value);
            if value.contains("$context.appId") {
                debug!("has context");
                let new_value = value.replace("$context.appId", &rpc_request.ctx.app_id);
                debug!("changed value {}", new_value);
                let _ = self.request.insert(new_value);
            } else {
                let _ = self.request.insert(value);
            }
        }
    }

    pub fn get_filter(&self, typ: RuleTransformType) -> Option<String> {
        match typ {
            RuleTransformType::Request => self.request.clone(),
            RuleTransformType::Event => self.event.clone(),
            RuleTransformType::Response => self.response.clone(),
        }
    }
}

pub enum RuleTransformType {
    Request,
    Response,
    Event,
}

#[derive(Debug, Clone, Default)]
pub struct RuleEngine {
    pub rules: RuleSet,
}

impl RuleEngine {
    fn build_path(path: &str, default_path: &str) -> String {
        if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("{}{}", default_path, path)
        }
    }

    pub fn build(extn_manifest: &ExtnManifest) -> Self {
        debug!("building rules engine {:?}", extn_manifest.rules_path);
        let mut engine = RuleEngine::default();
        for path in extn_manifest.rules_path.iter() {
            let path_for_rule = Self::build_path(path, &extn_manifest.default_path);
            debug!("loading rule {}", path_for_rule);
            if let Some(p) = Path::new(&path_for_rule).to_str() {
                if let Ok(contents) = fs::read_to_string(p) {
                    debug!("Rule content {}", contents);
                    if let Ok((path, rule_set)) = Self::load_from_content(contents) {
                        debug!("Rules loaded from path={}", path);
                        engine.rules.append(rule_set)
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

    pub fn load_from_content(contents: String) -> Result<(String, RuleSet), RippleError> {
        match serde_json::from_str::<RuleSet>(&contents) {
            Ok(manifest) => Ok((contents, manifest)),
            Err(err) => {
                warn!("{:?} could not load rule", err);
                Err(RippleError::InvalidInput)
            }
        }
    }

    pub fn has_rule(&self, request: &RpcRequest) -> bool {
        self.rules.rules.contains_key(&request.ctx.method)
    }

    pub fn get_rule(&self, rpc_request: &RpcRequest) -> Option<Rule> {
        println!("looking for rule {:?}", rpc_request.method);
        if let Some(mut rule) = self.rules.rules.get(&rpc_request.method).cloned() {
            rule.transform.apply_context(rpc_request);
            return Some(rule);
        }
        None
    }
}

pub fn jq_compile(input: Value, filter: &str, reference: String) -> Result<Value, RippleError> {
    debug!(
        "processing jq_rule={}, input {:?} , reference={}",
        filter, input, reference
    );
    let start = Utc::now().timestamp_millis();
    // start out only from core filters,
    // which do not include filters in the standard library
    // such as `map`, `select` etc.
    let mut defs = ParseCtx::new(Vec::new());

    // parse the filter
    let (f, errs) = jaq_parse::parse(filter, jaq_parse::main());
    if !errs.is_empty() {
        error!("Error in rule {:?}", errs);
        return Err(RippleError::RuleError);
    }

    // compile the filter in the context of the given definitions
    let f = defs.compile(f.unwrap());
    if !defs.errs.is_empty() {
        error!("Error in rule {}", reference);
        for (err, _) in defs.errs {
            error!("reference={} {}", reference, err);
        }
        return Err(RippleError::RuleError);
    }

    let inputs = RcIter::new(core::iter::empty());

    // iterator over the output values
    let out = f
        .run((Ctx::new([], &inputs), Val::from(input.clone())))
        .next();

    match out {
        Some(val) => match val {
            Ok(v) => {
                info!(
                    "Ripple Gateway Rule Processing Time: {},{}",
                    reference,
                    Utc::now().timestamp_millis() - start
                );
                trace!(
                    "jq_rule={}, input {:?} , extracted value={}",
                    filter,
                    input,
                    v
                );

                if v == Val::Null {
                    debug!(
                        "jq processing returned null for jq_rule={}, input {:?} , reference={}",
                        filter, input, reference
                    );
                    return Err(RippleError::InvalidValueReturned);
                }
                Ok(Value::from(v))
            }
            Err(e) => {
                debug!("Encountered primtive value in jq_rule={}, input {:?} , reference={}, error={}. Returning value {}", filter, input, reference,e,input);
                Err(RippleError::InvalidValueReturned)
            }
        },
        None => {
            error!(
                "Ripple Gateway Rule Processing Time: {},{}",
                reference,
                Utc::now().timestamp_millis() - start
            );
            Err(RippleError::RuleError)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ripple_sdk::{
        api::gateway::rpc_gateway_api::CallContext as Context,
        utils::{error::RippleError, logger::init_logger},
    };
    use serde_json::json;

    #[test]
    pub fn test_jq_compile_simple_return_value() {
        let input = serde_json::json!({
            "result": true
        });

        let filter = ".result";
        let reference = "test";
        let result = super::jq_compile(input.clone(), filter, reference.to_string());
        assert!(result.is_ok());
        assert!(result.unwrap().as_bool().unwrap());

        let result = super::jq_compile(input, ".broken", reference.to_string());
        assert_eq!(result, Err(RippleError::InvalidValueReturned));
    }
    #[test]
    pub fn test_jq_compile_complex_return_value() {
        let input = serde_json::json!(
            {
                    "result": true,
                    "success": true,
                    "foo": "baz"

            }
        );

        let filter = ".result";
        let reference = "test";
        let result = super::jq_compile(input.clone(), filter, reference.to_string());
        assert!(result.is_ok());
        assert!(result.unwrap().as_bool().unwrap());

        let result = super::jq_compile(input.clone(), ".broken", reference.to_string());
        assert_eq!(result, Err(RippleError::InvalidValueReturned));

        let result = super::jq_compile(input.clone(), ".foo", reference.to_string());
        assert_eq!(result, Ok("baz".into()));

        let result = super::jq_compile(input, ".success", reference.to_string());
        assert!(result.unwrap().as_bool().unwrap());
    }
    #[test]
    fn test_ruleset_append() {
        let mut ruleset1 = RuleSet {
            endpoints: HashMap::from([(
                "endpoint1".to_string(),
                RuleEndpoint {
                    protocol: RuleEndpointProtocol::Http,
                    url: "http://example.com".to_string(),
                    jsonrpc: true,
                },
            )]),
            rules: HashMap::from([(
                "rule1".to_string(),
                Rule {
                    alias: "alias1".to_string(),
                    transform: RuleTransform::default(),
                    endpoint: None,
                },
            )]),
        };

        let ruleset2 = RuleSet {
            endpoints: HashMap::from([(
                "endpoint2".to_string(),
                RuleEndpoint {
                    protocol: RuleEndpointProtocol::Websocket,
                    url: "ws://example.com".to_string(),
                    jsonrpc: false,
                },
            )]),
            rules: HashMap::from([(
                "rule2".to_string(),
                Rule {
                    alias: "alias2".to_string(),
                    transform: RuleTransform::default(),
                    endpoint: Some("endpoint2".to_string()),
                },
            )]),
        };

        ruleset1.append(ruleset2);

        assert_eq!(ruleset1.endpoints.len(), 2);
        assert_eq!(ruleset1.rules.len(), 2);
        assert!(ruleset1.endpoints.contains_key("endpoint1"));
        assert!(ruleset1.endpoints.contains_key("endpoint2"));
        assert!(ruleset1.rules.contains_key("rule1"));
        assert!(ruleset1.rules.contains_key("rule2"));
    }

    #[test]
    fn test_default_autostart() {
        assert!(default_autostart());
    }

    #[test]
    fn test_rule_transform_apply_context() {
        let mut transform = RuleTransform {
            request: Some("$context.appId/test".to_string()),
            response: None,
            event: None,
        };

        let rpc_request = RpcRequest {
            ctx: Context {
                app_id: "my_app".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        transform.apply_context(&rpc_request);

        assert_eq!(transform.request, Some("my_app/test".to_string()));

        // Test when request doesn't contain $context.appId
        let mut transform = RuleTransform {
            request: Some("no_context".to_string()),
            response: None,
            event: None,
        };
        transform.apply_context(&rpc_request);
        assert_eq!(transform.request, Some("no_context".to_string()));
    }

    #[test]
    fn test_rule_transform_get_filter() {
        let transform = RuleTransform {
            request: Some("request_filter".to_string()),
            response: Some("response_filter".to_string()),
            event: Some("event_filter".to_string()),
        };

        assert_eq!(
            transform.get_filter(RuleTransformType::Request),
            Some("request_filter".to_string())
        );
        assert_eq!(
            transform.get_filter(RuleTransformType::Response),
            Some("response_filter".to_string())
        );
        assert_eq!(
            transform.get_filter(RuleTransformType::Event),
            Some("event_filter".to_string())
        );
    }

    #[test]
    fn test_rule_engine_build_path() {
        assert_eq!(
            RuleEngine::build_path("/absolute/path", "/default/"),
            "/absolute/path"
        );
        assert_eq!(
            RuleEngine::build_path("relative/path", "/default/"),
            "/default/relative/path"
        );
    }

    #[test]
    fn test_rule_engine_has_rule() {
        let engine = RuleEngine {
            rules: RuleSet {
                endpoints: HashMap::new(),
                rules: HashMap::from([(
                    "test_method".to_string(),
                    Rule {
                        alias: "test_alias".to_string(),
                        transform: RuleTransform::default(),
                        endpoint: None,
                    },
                )]),
            },
        };

        let rpc_request = RpcRequest {
            ctx: Context {
                method: "test_method".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(engine.has_rule(&rpc_request));

        let rpc_request_no_rule = RpcRequest {
            ctx: Context {
                method: "non_existent_method".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(!engine.has_rule(&rpc_request_no_rule));
    }

    #[test]
    fn test_rule_engine_get_rule() {
        let engine = RuleEngine {
            rules: RuleSet {
                endpoints: HashMap::new(),
                rules: HashMap::from([(
                    "test_method".to_string(),
                    Rule {
                        alias: "test_alias".to_string(),
                        transform: RuleTransform {
                            request: Some("$context.appId/test".to_string()),
                            response: None,
                            event: None,
                        },
                        endpoint: None,
                    },
                )]),
            },
        };

        let rpc_request = RpcRequest {
            ctx: Context {
                method: "test_method".to_string(),
                app_id: "my_app".to_string(),
                ..Default::default()
            },
            method: "test_method".to_string(),
            ..Default::default()
        };
        println!("{:?}", engine.rules.rules);
        let rule = engine.get_rule(&rpc_request);
        assert!(rule.is_some());
        let rule = rule.unwrap();
        assert_eq!(rule.alias, "test_alias");
        assert_eq!(rule.transform.request, Some("my_app/test".to_string()));

        let rpc_request_no_rule = RpcRequest {
            ctx: Context {
                method: "non_existent_method".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(engine.get_rule(&rpc_request_no_rule).is_none());
    }

    #[test]
    fn test_rule_engine_load_from_content() {
        let valid_content = r#"
        {
            "endpoints": {
                "test_endpoint": {
                    "protocol": "http",
                    "url": "http://example.com"
                }
            },
            "rules": {
                "test_rule": {
                    "alias": "test_alias",
                    "transform": {
                        "request": "request_filter"
                    },
                    "endpoint": "test_endpoint"
                }
            }
        }"#;

        let result = RuleEngine::load_from_content(valid_content.to_string());
        assert!(result.is_ok());
        let (content, ruleset) = result.unwrap();
        assert_eq!(content, valid_content);
        assert_eq!(ruleset.endpoints.len(), 1);
        assert_eq!(ruleset.rules.len(), 1);

        let invalid_content = r#"
        {
            "invalid": "json"
        }"#;

        let result = RuleEngine::load_from_content(invalid_content.to_string());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RippleError::InvalidInput));
    }

    #[test]
    fn test_jq_compile_edge_cases() {
        // Test with empty input
        let _ = init_logger("test_jq_compile_edge_cases".into());
        let empty_input = json!({});
        let filter = ".";
        let reference = "empty_input".to_string();
        let result = jq_compile(empty_input, filter, reference);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), json!({}));

        // Test with array input
        let array_input = json!([1, 2, 3]);
        let filter = ".[0]";
        let reference = "array_input".to_string();
        let result = jq_compile(array_input, filter, reference);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), json!(1));

        // Test with invalid filter
        let input = json!({"a": 1});
        let filter = ".b[";
        let reference = "invalid_filter".to_string();
        let result = jq_compile(input, filter, reference);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RippleError::RuleError));

        // Test with filter that returns empty output
        let input = json!({"a": 1});
        let filter = ".b[]";
        let reference = "empty_output".to_string();
        let result = jq_compile(input, filter, reference);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RippleError::InvalidValueReturned
        ));
    }
}
