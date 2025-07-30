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
    async_read, async_read_async, async_write, async_write_async,
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
        async_read!(self.device_session_id, |device_session_id| {
            device_session_id.clone().unwrap_or_default().clone()
        })
    }

    pub async fn operational_telemetry_listener(&self, target: &str, listen: bool) {
        async_write!(self.operational_telemetry_listeners, |listeners| {
            if listen {
                listeners.insert(target.to_string());
            } else {
                listeners.remove(target);
            }
        });
    }

    pub async fn get_listeners(&self) -> Vec<String> {
        async_read!(self.operational_telemetry_listeners, |listeners| {
            listeners.iter().map(|x| x.to_owned()).collect()
        })
    }

    pub async fn update_session_id(&self, value: Option<String>) {
        async_write!(self.device_session_id, |device_session_id| {
            let _ = device_session_id.insert(value.unwrap_or_default());
        });
    }

    pub async fn add_api_stats(&self, request_id: &str, api: &str) {
        let size = async_write!(self.api_stats_map, |map| {
            map.insert(request_id.to_string(), ApiStats::new(api.into()));
            map.len()
        });

        if size >= API_STATS_MAP_SIZE_WARNING {
            warn!("add_api_stats: api_stats_map size warning: {}", size);
        }
    }

    pub async fn remove_api_stats(&mut self, request_id: &str) {
        async_write!(self.api_stats_map, |map| {
            map.remove(request_id);
        });
    }

    pub async fn update_api_stats_ref(&mut self, request_id: &str, stats_ref: Option<String>) {
        async_write!(self.api_stats_map, |map| {
            if let Some(stats) = map.get_mut(request_id) {
                stats.stats_ref = stats_ref;
            } else {
                println!(
                    "update_api_stats_ref: request_id not found: request_id={}",
                    request_id
                );
            }
        });
    }

    pub async fn update_api_stage(&mut self, request_id: &str, stage: &str) -> i64 {
        async_write!(self.api_stats_map, |map| {
            if let Some(stats) = map.get_mut(request_id) {
                stats.stats.update_stage(stage)
            } else {
                error!(
                    "update_api_stage: request_id not found: request_id={}",
                    request_id
                );
                -1
            }
        })
    }

    pub async fn get_api_stats(&self, request_id: &str) -> Option<ApiStats> {
        async_read!(self.api_stats_map, |map| { map.get(request_id).cloned() })
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
        // ops_metrics
        //     .write()
        //     .await
        //     .add_api_stats(request_id, api)
        //     .await;
        async_write_async!(ops_metrics, |metrics| {
            metrics.add_api_stats(request_id, api).await;
        });
    }

    pub async fn remove_api_stats(ops_metrics: Arc<RwLock<OpMetricState>>, request_id: &str) {
        async_write_async!(ops_metrics, |metrics| {
            metrics.remove_api_stats(request_id).await;
        });
    }

    pub async fn update_api_stats_ref(
        ops_metrics: Arc<RwLock<OpMetricState>>,
        request_id: &str,
        stats_ref: Option<String>,
    ) {
        async_write_async!(ops_metrics, |metrics| {
            let mut stats_ref_map = metrics.api_stats_map.write().await;
            if let Some(stats) = stats_ref_map.get_mut(request_id) {
                stats.stats_ref = stats_ref;
            } else {
                println!(
                    "update_api_stats_ref: request_id not found: request_id={}",
                    request_id
                );
            }
            drop(stats_ref_map);
        });
    }
    pub async fn get_api_stats(
        ops_metrics: Arc<RwLock<OpMetricState>>,
        request_id: &str,
    ) -> Option<ApiStats> {
        async_read_async!(ops_metrics, |metrics| {
            let api_stats = metrics.api_stats_map.read().await;
            api_stats.get(request_id).cloned()
        })
    }

    pub async fn update_api_stage(
        ops_metrics: Arc<RwLock<OpMetricState>>,
        request_id: &str,
        stage: &str,
    ) -> i64 {
        async_write_async!(ops_metrics, |metrics| {
            let mut api_stats_map = metrics.api_stats_map.write().await;
            let updated = if let Some(stats) = api_stats_map.get_mut(request_id) {
                stats.stats.update_stage(stage)
            } else {
                error!(
                    "update_api_stage: request_id not found: request_id={}",
                    request_id
                );
                -1
            };

            drop(api_stats_map);
            updated
        })
    }
    pub async fn get_device_session_id(ops_metrics: Arc<RwLock<OpMetricState>>) -> String {
        async_read_async!(ops_metrics, |metrics| {
            let device_session_id_guard = metrics.device_session_id.clone();
            let device_session_id_guard = device_session_id_guard.read().await;
            let device_session_id = device_session_id_guard.as_ref();
            let returned = device_session_id.cloned().unwrap_or_default();
            drop(device_session_id_guard);
            returned
        })
    }

    pub async fn update_session_id(ops_metrics: Arc<RwLock<OpMetricState>>, value: Option<String>) {
        let value = value.unwrap_or_default();
        async_write_async!(ops_metrics, |metrics| {
            let mut session = metrics.device_session_id.write().await;
            let _ = session.insert(value);
            drop(session);
        });
    }
    pub async fn get_listeners(ops_metrics: Arc<RwLock<OpMetricState>>) -> Vec<String> {
        async_read_async!(ops_metrics, |metrics| { metrics.get_listeners().await })
    }

    pub async fn operational_telemetry_listener(
        ops_metrics: Arc<RwLock<OpMetricState>>,
        target: &str,
        listen: bool,
    ) {
        async_write_async!(ops_metrics, |metrics| {
            let mut listeners = metrics.operational_telemetry_listeners.write().await;
            if listen {
                listeners.insert(target.to_string());
            } else {
                listeners.remove(target);
            }
            drop(listeners);
        })
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
