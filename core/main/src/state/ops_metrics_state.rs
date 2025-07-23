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
    sync::Arc,
};

use ripple_sdk::{
    api::observability::metrics_util::ApiStats,
    async_read_lock, async_write_lock,
    chrono::{DateTime, Utc},
    log::{error, warn},
    tokio::sync::RwLock,
};

include!(concat!(env!("OUT_DIR"), "/version.rs"));

const API_STATS_MAP_SIZE_WARNING: usize = 10;

#[derive(Debug, Clone, Default)]
pub struct OpMetricState {
    pub start_time: DateTime<Utc>,
    operational_telemetry_listeners: Arc<RwLock<HashSet<String>>>,
    pub api_stats_map: Arc<RwLock<HashMap<String, ApiStats>>>,
    device_session_id: Arc<RwLock<Option<String>>>,
}

impl OpMetricState {
    pub async fn get_device_session_id(&self) -> String {
        {
            let device_session_id = async_read_lock!(self.device_session_id);
            device_session_id.clone().unwrap_or_default().clone()
        }
    }

    pub async fn operational_telemetry_listener(&self, target: &str, listen: bool) {
        let mut listeners = self.operational_telemetry_listeners.write().await;
        if listen {
            listeners.insert(target.to_string());
        } else {
            listeners.remove(target);
        }
    }

    pub async fn get_listeners(&self) -> Vec<String> {
        self.operational_telemetry_listeners
            .read()
            .await
            .iter()
            .map(|x| x.to_owned())
            .collect()
    }

    pub async fn update_session_id(&self, value: Option<String>) {
        let value = value.unwrap_or_default();
        {
            let mut context = self.device_session_id.write().await;
            let _ = context.insert(value);
        }
    }

    pub async fn add_api_stats(&self, request_id: &str, api: &str) {
        let mut api_stats_map = self.api_stats_map.write().await;
        api_stats_map.insert(request_id.to_string(), ApiStats::new(api.into()));

        let size = api_stats_map.len();
        if size >= API_STATS_MAP_SIZE_WARNING {
            warn!("add_api_stats: api_stats_map size warning: {}", size);
        }
    }

    pub async fn remove_api_stats(&mut self, request_id: &str) {
        let mut api_stats_map = self.api_stats_map.write().await;
        api_stats_map.remove(request_id);
    }

    pub async fn update_api_stats_ref(&mut self, request_id: &str, stats_ref: Option<String>) {
        let mut api_stats_map = self.api_stats_map.write().await;
        if let Some(stats) = api_stats_map.get_mut(request_id) {
            stats.stats_ref = stats_ref;
        } else {
            println!(
                "update_api_stats_ref: request_id not found: request_id={}",
                request_id
            );
        }
    }

    pub async fn update_api_stage(&mut self, request_id: &str, stage: &str) -> i64 {
        let mut api_stats_map = async_write_lock!(self.api_stats_map);
        if let Some(stats) = api_stats_map.get_mut(request_id) {
            stats.stats.update_stage(stage)
        } else {
            error!(
                "update_api_stage: request_id not found: request_id={}",
                request_id
            );
            -1
        }
    }

    pub async fn get_api_stats(&self, request_id: &str) -> Option<ApiStats> {
        let api_stats_map = async_read_lock!(self.api_stats_map);
        api_stats_map.get(request_id).cloned()
    }
}
pub struct OpsMetrics {}
impl OpsMetrics {
    /*
    free functions to enabled better synchronization */
    pub async fn add_api_stats(
        ops_metrics: Arc<RwLock<OpMetricState>>,
        request_id: &str,
        api: &str,
    ) {
        ops_metrics
            .write()
            .await
            .add_api_stats(request_id, api)
            .await;
    }

    pub async fn remove_api_stats(ops_metrics: Arc<RwLock<OpMetricState>>, request_id: &str) {
        let api_stats_map = ops_metrics.write().await;
        let mut api_stats_map = api_stats_map.api_stats_map.write().await;
        api_stats_map.remove(request_id);
    }

    pub async fn update_api_stats_ref(
        ops_metrics: Arc<RwLock<OpMetricState>>,
        request_id: &str,
        stats_ref: Option<String>,
    ) {
        let api_stats_map = ops_metrics.write().await;
        let mut api_stats_map = api_stats_map.api_stats_map.write().await;
        if let Some(stats) = api_stats_map.get_mut(request_id) {
            stats.stats_ref = stats_ref;
        } else {
            println!(
                "update_api_stats_ref: request_id not found: request_id={}",
                request_id
            );
        }
    }
    pub async fn get_api_stats(
        ops_metrics: Arc<RwLock<OpMetricState>>,
        request_id: &str,
    ) -> Option<ApiStats> {
        let api_stats_map = ops_metrics.write().await;
        let api_stats_map = api_stats_map.api_stats_map.write().await;
        api_stats_map.get(request_id).cloned()
    }

    pub async fn update_api_stage(
        ops_metrics: Arc<RwLock<OpMetricState>>,
        request_id: &str,
        stage: &str,
    ) -> i64 {
        let api_stats_map = ops_metrics.write().await;
        let mut api_stats_map = api_stats_map.api_stats_map.write().await;

        if let Some(stats) = api_stats_map.get_mut(request_id) {
            stats.stats.update_stage(stage)
        } else {
            error!(
                "update_api_stage: request_id not found: request_id={}",
                request_id
            );
            -1
        }
    }
    pub async fn get_device_session_id(ops_metrics: Arc<RwLock<OpMetricState>>) -> String {
        let session = ops_metrics.read().await;
        let session = session.device_session_id.read().await.clone();
        let session = session.unwrap_or_default();
        session
    }

    pub async fn update_session_id(ops_metrics: Arc<RwLock<OpMetricState>>, value: Option<String>) {
        let value = value.unwrap_or_default();
        {
            let session = ops_metrics.write().await;
            let mut session = session.device_session_id.write().await;
            let _ = session.insert(value);
        }
    }
    pub async fn get_listeners(ops_metrics: Arc<RwLock<OpMetricState>>) -> Vec<String> {
        ops_metrics.read().await.get_listeners().await
    }

    pub async fn operational_telemetry_listener(
        ops_metrics: Arc<RwLock<OpMetricState>>,
        target: &str,
        listen: bool,
    ) {
        /*
        yuck
        */
        let mut listeners = async_write_lock!(ops_metrics)
            .clone()
            .operational_telemetry_listeners
            .write()
            .await
            .clone();
        if listen {
            listeners.insert(target.to_string());
        } else {
            listeners.remove(target);
        }
        drop(listeners);
    }
}
#[macro_export]
macro_rules! op_metric_state_default {
    () => {
        std::sync::Arc::new(ripple_sdk::tokio::sync::RwLock::new(
            OpMetricState::default(),
        ))
    };
}
