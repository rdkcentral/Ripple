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

use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

use ripple_sdk::chrono::{DateTime, Utc};

include!(concat!(env!("OUT_DIR"), "/version.rs"));

#[derive(Debug, Clone, Default)]
pub struct OpMetricState {
    pub start_time: DateTime<Utc>,
    operational_telemetry_listeners: Arc<RwLock<HashSet<String>>>,
    device_session_id: Arc<RwLock<Option<String>>>,
}

impl OpMetricState {
    pub fn get_device_session_id(&self) -> String {
        self.device_session_id
            .read()
            .unwrap()
            .clone()
            .unwrap_or_default()
    }

    pub fn operational_telemetry_listener(&self, target: &str, listen: bool) {
        let mut listeners = self.operational_telemetry_listeners.write().unwrap();
        if listen {
            listeners.insert(target.to_string());
        } else {
            listeners.remove(target);
        }
    }

    pub fn get_listeners(&self) -> Vec<String> {
        self.operational_telemetry_listeners
            .read()
            .unwrap()
            .iter()
            .map(|x| x.to_owned())
            .collect()
    }

    pub fn update_session_id(&self, value: Option<String>) {
        let value = value.unwrap_or_default();
        {
            let mut context = self.device_session_id.write().unwrap();
            let _ = context.insert(value);
        }
    }
}
