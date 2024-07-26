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

use std::collections::HashMap;

use ripple_sdk::utils::error::RippleError;
use serde_json::Value;
use std::sync::{Arc, Mutex};

type UtilityFunction =
    Arc<dyn Fn(Option<Value>) -> Result<Option<Value>, RippleError> + Send + Sync>;
pub struct EventManagementUtility {
    pub functions: Mutex<HashMap<String, UtilityFunction>>,
}

impl EventManagementUtility {
    pub fn new() -> Self {
        Self {
            functions: Mutex::new(HashMap::new()),
        }
    }
    pub fn register_custom_functions(&self) {
        // Add a custom function to event utility based on the tag
        self.register_function(
            "AdvertisingSetRestrictionEventDecorator".to_string(),
            Arc::new(EventManagementUtility::advertising_set_restriction_event_decorator),
        );
        self.register_function(
            "AdvertisingPolicyEventDecorator".to_string(),
            Arc::new(EventManagementUtility::advertising_policy_event_decorator),
        );
    }
    pub fn register_function(&self, name: String, function: UtilityFunction) {
        self.functions.lock().unwrap().insert(name, function);
    }

    pub fn get_function(&self, name: &str) -> Option<UtilityFunction> {
        self.functions.lock().unwrap().get(name).cloned()
    }

    // Utility functions used in Rule Engine
    pub fn advertising_set_restriction_event_decorator(
        value: Option<Value>,
    ) -> Result<Option<Value>, RippleError> {
        println!("advertising_set_restriction_event_decorator {:?}", value);
        Ok(value)
    }
    pub fn advertising_policy_event_decorator(
        value: Option<Value>,
    ) -> Result<Option<Value>, RippleError> {
        println!("advertising_policy_event_decorator {:?}", value);
        // change the value to something else
        let modified_value = Some(Value::String("Decorated Event Value".to_string()));
        Ok(modified_value)
    }
}
