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
