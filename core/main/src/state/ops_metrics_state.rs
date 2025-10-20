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
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::observability::metrics_util::ApiStats,
    chrono::{DateTime, Utc},
    log::trace,
};

include!(concat!(env!("OUT_DIR"), "/version.rs"));

const API_STATS_MAP_SIZE_WARNING: usize = 10;

#[derive(Debug, Clone, Default)]
pub struct OpMetricState {
    pub start_time: DateTime<Utc>,
    operational_telemetry_listeners: Arc<RwLock<HashSet<String>>>,
    api_stats_map: Arc<RwLock<HashMap<String, ApiStats>>>,
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

    pub fn add_api_stats(&self, request_id: &str, api: &str) {
        let mut api_stats_map = self.api_stats_map.write().unwrap();
        api_stats_map.insert(request_id.to_string(), ApiStats::new(api.into()));

        let size = api_stats_map.len();
        if size >= API_STATS_MAP_SIZE_WARNING {
            trace!("add_api_stats: api_stats_map size warning: {}", size);
        }
    }

    pub fn remove_api_stats(&mut self, request_id: &str) {
        let mut api_stats_map = self.api_stats_map.write().unwrap();
        api_stats_map.remove(request_id);
    }

    pub fn update_api_stats_ref(&mut self, request_id: &str, stats_ref: Option<String>) {
        let mut api_stats_map = self.api_stats_map.write().unwrap();
        if let Some(stats) = api_stats_map.get_mut(request_id) {
            stats.stats_ref = stats_ref;
        } else {
            trace!(
                "update_api_stats_ref: request_id not found: request_id={}",
                request_id
            );
        }
    }

    pub fn update_api_stage(&mut self, request_id: &str, stage: &str) -> i64 {
        let mut api_stats_map = self.api_stats_map.write().unwrap();
        if let Some(stats) = api_stats_map.get_mut(request_id) {
            stats.stats.update_stage(stage)
        } else {
            trace!(
                "update_api_stage: request_id not found: request_id={}",
                request_id
            );
            -1
        }
    }

    pub fn get_api_stats(&self, request_id: &str) -> Option<ApiStats> {
        let api_stats_map = self.api_stats_map.read().unwrap();
        api_stats_map.get(request_id).cloned()
    }
}
