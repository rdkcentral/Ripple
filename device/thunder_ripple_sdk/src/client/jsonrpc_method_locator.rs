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

use std::str::FromStr;

#[derive(Debug, Default)]
pub struct JsonRpcMethodLocator {
    pub module: Option<String>,
    pub version: Option<String>,
    pub method_name: String,
    pub qualifier: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RpcMethodLocatorParseError {}

impl JsonRpcMethodLocator {}

impl FromStr for JsonRpcMethodLocator {
    type Err = RpcMethodLocatorParseError;
    /// Parses a string into a locator object
    /// org.rdk.Controller.1.status@org.rdk.Network
    /// Would result into {
    ///    module: org.rdk.Controller,
    ///    version: 1,
    ///    method_name: status
    ///    qualifier: org.rdk.Network
    /// }
    /// Need to account for edge cases where module name can have any number of periods
    /// There might be no version
    /// There might be no qualifier
    /// There might only be a method_name
    fn from_str(s: &str) -> Result<JsonRpcMethodLocator, RpcMethodLocatorParseError> {
        let mut parts = s.split('@').collect::<Vec<&str>>();
        let q = match parts.len() {
            1 => None,
            2 => parts.pop(),
            _ => {
                return Err(RpcMethodLocatorParseError {});
            }
        };
        if let Some(v) = parts.first() {
            let rest = String::from(*v);
            let mut parts = rest.split('.').collect::<Vec<&str>>();
            return match parts.len() {
                1 => Ok(JsonRpcMethodLocator {
                    module: None,
                    version: None,
                    qualifier: q.map(str::to_string),
                    method_name: rest.to_string(),
                }),
                2 => Ok(JsonRpcMethodLocator {
                    method_name: parts.last().unwrap().to_string(),
                    module: Some(parts.get(parts.len() - 2).unwrap().to_string()),
                    version: None,
                    qualifier: q.map(str::to_string),
                }),
                _ => {
                    let mn = parts.pop().unwrap();
                    let v_or_mod = parts.pop().unwrap();
                    let mut v = Some(v_or_mod);
                    if v_or_mod.parse::<i32>().is_err() {
                        v = None;
                        parts.push(v_or_mod);
                    }
                    let md = Some(parts.join("."));
                    Ok(JsonRpcMethodLocator {
                        module: md,
                        version: v.map(str::to_string),
                        qualifier: q.map(str::to_string),
                        method_name: mn.to_string(),
                    })
                }
            };
        }
        Err(RpcMethodLocatorParseError {})
    }
}

#[cfg(test)]
mod tests {
    use super::{JsonRpcMethodLocator, RpcMethodLocatorParseError};
    use std::str::FromStr;

    #[test]
    pub fn test_json_rpc_method_locator() {
        let locator =
            JsonRpcMethodLocator::from_str("org.rdk.Controller.1.status@org.rdk.Network").unwrap();
        assert_eq!(locator.module.unwrap(), String::from("org.rdk.Controller"));
        assert_eq!(locator.version.unwrap(), String::from("1"));
        assert_eq!(locator.method_name, String::from("status"));
        assert_eq!(locator.qualifier.unwrap(), String::from("org.rdk.Network"));

        let locator =
            JsonRpcMethodLocator::from_str("org.rdk.Controller.status@org.rdk.Network").unwrap();
        assert_eq!(locator.module.unwrap(), String::from("org.rdk.Controller"));
        assert_eq!(locator.version, None);
        assert_eq!(locator.method_name, String::from("status"));
        assert_eq!(locator.qualifier.unwrap(), String::from("org.rdk.Network"));

        let locator = JsonRpcMethodLocator::from_str("org.rdk.Controller.status").unwrap();
        assert_eq!(locator.module.unwrap(), String::from("org.rdk.Controller"));
        assert_eq!(locator.version, None);
        assert_eq!(locator.method_name, String::from("status"));
        assert_eq!(locator.qualifier, None);

        let locator = JsonRpcMethodLocator::from_str("Controller.status").unwrap();
        assert_eq!(locator.module.unwrap(), String::from("Controller"));
        assert_eq!(locator.version, None);
        assert_eq!(locator.method_name, String::from("status"));
        assert_eq!(locator.qualifier, None);

        let locator = JsonRpcMethodLocator::from_str("status").unwrap();
        assert_eq!(locator.module, None);
        assert_eq!(locator.version, None);
        assert_eq!(locator.method_name, String::from("status"));
        assert_eq!(locator.qualifier, None);

        let locator = JsonRpcMethodLocator {
            module: None,
            version: None,
            method_name: "".to_owned(),
            qualifier: None,
        };
        assert_eq!(locator.module, None);
        assert_eq!(locator.version, None);
        assert!(locator.method_name.is_empty());
        assert_eq!(locator.qualifier, None);
    }
    #[test]
    pub fn test_json_rpc_method_locator_parse_module_and_methods() {
        let locator = JsonRpcMethodLocator::from_str("org.test.threadpool.1.register").unwrap();
        assert_eq!(locator.module.unwrap(), String::from("org.test.threadpool"));
        assert_eq!(locator.version.unwrap(), String::from("1"));
        assert_eq!(locator.method_name, String::from("register"));
        assert_eq!(locator.qualifier, None);

        let locator = JsonRpcMethodLocator::from_str("org.test.threadpool.1.unregister").unwrap();
        assert_eq!(locator.module.unwrap(), String::from("org.test.threadpool"));
        assert_eq!(locator.version.unwrap(), String::from("1"));
        assert_eq!(locator.method_name, String::from("unregister"));
        assert_eq!(locator.qualifier, None);

        let locator = JsonRpcMethodLocator::from_str("org.test.threadpool.1.testMethod").unwrap();
        assert_eq!(locator.module.unwrap(), String::from("org.test.threadpool"));
        assert_eq!(locator.version.unwrap(), String::from("1"));
        assert_eq!(locator.method_name, String::from("testMethod"));
        assert_eq!(locator.qualifier, None);

        let locator = JsonRpcMethodLocator::from_str("testMethod").unwrap();
        assert_eq!(locator.module, None);
        assert_eq!(locator.version, None);
        assert_eq!(locator.method_name, String::from("testMethod"));
        assert_eq!(locator.qualifier, None);
    }
    #[test]
    pub fn neg_test_json_rpc_method_locator() {
        let locator_err =
            JsonRpcMethodLocator::from_str("org.rdk.Controller.status@org.rdk.Network@test3@test4")
                .unwrap_err();
        assert_eq!(locator_err, RpcMethodLocatorParseError {});
    }
}
