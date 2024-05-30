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
use ripple_sdk::api::{
    gateway::rpc_gateway_api::RpcRequest, manifest::extn_manifest::ExtnManifest,
};
use ripple_sdk::{
    log::{debug, warn},
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
            if value.contains("$context.appId") {
                let _ = value.replace("$context.appId", &rpc_request.ctx.method);
            }
            let _ = self.request.insert(value);
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
    pub fn build(extn_manifest: &ExtnManifest) -> Self {
        let mut engine = RuleEngine::default();
        for path in extn_manifest.rules_path.iter() {
            if let Some(p) = Path::new(path).to_str() {
                if let Ok(contents) = fs::read_to_string(p) {
                    if let Ok((path, rule_set)) = Self::load_from_content(contents) {
                        debug!("Rules loaded from path={}", path);
                        engine.rules.append(rule_set)
                    }
                }
            }
        }
        engine
    }

    pub fn load_from_content(contents: String) -> Result<(String, RuleSet), RippleError> {
        match serde_json::from_str::<RuleSet>(&contents) {
            Ok(manifest) => Ok((contents, manifest)),
            Err(err) => {
                warn!("{:?} could not load device manifest", err);
                Err(RippleError::InvalidInput)
            }
        }
    }

    pub fn has_rule(&self, request: &RpcRequest) -> bool {
        self.rules.rules.contains_key(&request.ctx.method)
    }

    pub fn get_rule(&self, rpc_request: &RpcRequest) -> Option<Rule> {
        if let Some(mut rule) = self.rules.rules.get(&rpc_request.method).cloned() {
            rule.transform.apply_context(rpc_request);
            return Some(rule);
        }
        None
    }
}
