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
        let rules: HashMap<String, Rule> = rule_set
            .rules
            .into_iter()
            .map(|(k, v)| {
                debug!("Loading JQ Rule for {}", k.to_lowercase());
                (k.to_lowercase(), v)
            })
            .collect();
        self.rules.extend(rules);
    }
}

#[derive(Debug, Deserialize, Clone)]
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
                    if let Ok((_, rule_set)) = Self::load_from_content(contents) {
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
        self.rules
            .rules
            .contains_key(&request.ctx.method.to_lowercase())
    }

    pub fn get_rule(&self, rpc_request: &RpcRequest) -> Option<Rule> {
        if let Some(mut rule) = self
            .rules
            .rules
            .get(&rpc_request.method.to_lowercase())
            .cloned()
        {
            rule.transform.apply_context(rpc_request);
            return Some(rule);
        } else {
            info!("Rule not available for {}", rpc_request.method);
        }
        None
    }
}

pub fn jq_compile(input: Value, filter: &str, reference: String) -> Result<Value, RippleError> {
    debug!("Jq rule {}  input {:?}", filter, input);
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
