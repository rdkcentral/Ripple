// Copyright 2025 Comcast Cable Communications Management, LLC
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
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub fn load_internal_api_map() -> Arc<HashMap<String, Value>> {
    match std::fs::read_to_string("internal_apis.json") {
        Ok(data) => match serde_json::from_str(&data) {
            Ok(parsed) => Arc::new(parsed),
            Err(e) => {
                eprintln!("[AppGW] Failed to parse internal_apis.json: {e}");
                Arc::new(HashMap::new())
            }
        },
        Err(_) => {
            println!("[AppGW] internal_apis.json not found. Skipping internal API mapping.");
            Arc::new(HashMap::new())
        }
    }
}

pub fn load_service_rules() -> HashMap<String, String> {
    let file_content =
        std::fs::read_to_string("mock_service.rules").expect("Failed to read rules file");
    let rules: Value = serde_json::from_str(&file_content).expect("Failed to parse rules file");

    let mut method_to_service_map = HashMap::new();
    if let Some(rules_map) = rules["rules"].as_object() {
        for (method_pattern, rule) in rules_map {
            if let Some(alias) = rule["alias"].as_str() {
                method_to_service_map.insert(method_pattern.clone(), alias.to_string());
            }
        }
    }

    method_to_service_map
}
