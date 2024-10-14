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
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Arc;

use ripple_sdk::tokio::net::TcpStream;
use ripple_sdk::{
    log::{debug, error, info},
    tokio::{
        self,
        sync::mpsc::{self, Receiver, Sender},
        sync::Mutex,
        time::{timeout, Duration},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::broker::endpoint_broker::{
    BrokerCallback, BrokerOutput, BrokerRequest, EndpointBrokerState,
};
use futures::stream::SplitSink;
use futures_util::SinkExt;

use crate::broker::thunder_broker::ThunderBroker;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
const USER_DATA_MIGRATION_CONFIG_FILE_NAME: &str = "user_data_migration_config.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MigrationConfigEntry {
    namespace: String,
    key: String,
    default: Value,
    getter: String,
    setter: String,
    migrated: bool,
}

type MigrationMap = HashMap<String, MigrationConfigEntry>;
// This struct is responsible for migrating user data from the legacy storage to the new storage.
#[derive(Clone, Debug)]
pub struct UserDataMigrator {
    migration_config: Arc<Mutex<MigrationMap>>, // persistent migration map
    config_file_path: String,                   // path to the migration map file
    response_tx: Sender<BrokerOutput>,
    response_rx: Arc<Mutex<Receiver<BrokerOutput>>>,
}

impl UserDataMigrator {
    pub fn create() -> Option<Self> {
        let possible_config_file_paths = vec![
            format!("/etc/{}", USER_DATA_MIGRATION_CONFIG_FILE_NAME),
            format!(
                "/opt/persistent/ripple/{}",
                USER_DATA_MIGRATION_CONFIG_FILE_NAME
            ),
            format!("./{}", USER_DATA_MIGRATION_CONFIG_FILE_NAME),
        ];

        for path in possible_config_file_paths {
            if Path::new(&path).exists() {
                debug!("Found migration map file: {}", path);
                if let Some(migration_map) = Self::load_migration_config(&path) {
                    let (response_tx, response_rx) = mpsc::channel(16);
                    return Some(UserDataMigrator {
                        migration_config: Arc::new(Mutex::new(migration_map)),
                        config_file_path: path.to_string(),
                        response_tx,
                        response_rx: Arc::new(Mutex::new(response_rx)),
                    });
                }
            }
        }
        debug!("No migration map file found");
        None
    }

    async fn get_matching_migration_entry_on_method(
        &self,
        method: &str,
    ) -> Option<MigrationConfigEntry> {
        let migration_map = self.migration_config.lock().await;
        migration_map
            .values()
            .find(|entry| entry.getter == method || entry.setter == method)
            .cloned()
    }

    // function to intercept and handle broker request. Perform migration if needed
    pub async fn intercept_broker_request(
        &self,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        request: &mut BrokerRequest,
    ) -> (bool, Option<Value>) {
        let method = request.rpc.method.clone();
        if let Some(config_entry) = self.get_matching_migration_entry_on_method(&method).await {
            // migration entry found for either getter or setter method
            // for setter case, irrespective of the migration status, update the new value in the new storage and sync
            // with the legacy storage

            if config_entry.setter == method {
                // perform the setter update and sync up logic asynchronously
                // update legacy storage with the new value as fire and forget operation
                self.set_migration_status(&config_entry.namespace, &config_entry.key)
                    .await;
                // TBD: apply transform rule if any and get the params.
                self.write_to_legacy_storage(
                    &config_entry.namespace,
                    &config_entry.key,
                    &broker,
                    ws_tx.clone(),
                    &request,
                    &config_entry.default,
                )
                .await;
                // returning false to continue with the original setter request
                return (false, None);
            } else {
                // perform the getter migration logic asynchronously
                if !config_entry.migrated {
                    let migrated_value = self
                        .perform_getter_migration(&broker, &request, &config_entry)
                        .await;
                    return (false, Some(migrated_value));
                } else {
                    // the migration is already done, continue with the original request
                    return (false, None);
                }
            }
        }

        // continue with the original request
        (false, None)
    }

    async fn write_to_legacy_storage(
        &self,
        namespace: &str,
        key: &str,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        _request: &BrokerRequest,
        value: &Value,
    ) {
        let request_id = EndpointBrokerState::get_next_id();
        let call_sign = "org.rdk.PersistentStore.1.".to_owned();
        let thunder_request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": format!("{}setValue", call_sign),
            "params": json!({
                "namespace": namespace,
                "key": key,
                "value": value.to_string(),
                "scope": "device",
            })
        })
        .to_string();

        // Register custom callback to handle the response
        broker
            .register_custom_callback(
                request_id,
                BrokerCallback {
                    sender: self.response_tx.clone(),
                },
            )
            .await;

        // send the request to the legacy storage
        if let Err(e) = self.send_thunder_request(&ws_tx, &thunder_request).await {
            error!("Failed to send thunder request: {:?}", e);
            return;
        }

        // Spawn a task to wait for the response
        let response_rx = self.response_rx.clone();
        let broker_clone = broker.clone();
        tokio::spawn(async move {
            if let Err(e) =
                UserDataMigrator::wait_for_response(response_rx, broker_clone, request_id).await
            {
                error!("Error waiting for response: {:?}", e);
            }
        });
    }

    async fn send_thunder_request(
        &self,
        ws_tx: &Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        request: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut ws_tx = ws_tx.lock().await;
        ws_tx.feed(Message::Text(request.to_string())).await?;
        ws_tx.flush().await?;
        Ok(())
    }

    async fn wait_for_response(
        response_rx: Arc<Mutex<tokio::sync::mpsc::Receiver<BrokerOutput>>>,
        broker: ThunderBroker,
        request_id: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut response_rx = response_rx.lock().await;
        match timeout(Duration::from_secs(30), response_rx.recv()).await {
            Ok(Some(response)) => {
                info!(
                    "Received response at custom write_to_legacy_storage: {:?}",
                    response
                );
            }
            Ok(None) => {
                error!("Failed to receive response");
            }
            Err(_) => {
                error!("Timeout waiting for response");
            }
        }
        broker.unregister_custom_callback(request_id).await;
        Ok(())
    }
    // function to perform the getter migration logic asynchronously
    async fn perform_getter_migration(
        &self,
        broker: &ThunderBroker,
        request: &BrokerRequest,
        config_entry: &MigrationConfigEntry,
    ) -> Value {
        let mut new_storage_value = Value::Null;
        // Get the value from the new storage
        //new_storage_value = self.get_new_storage_value(&broker, &request).await;
        new_storage_value
    }

    // function to set the migration flag to true and update the migration map in the config file
    async fn set_migration_status(&self, namespace: &str, key: &str) {
        let mut config_entry_changed = false;
        {
            let mut migration_map = self.migration_config.lock().await;
            if let Some(mut config_entry) = migration_map
                .values_mut()
                .find(|entry| entry.namespace == namespace && entry.key == key)
            {
                if !config_entry.migrated {
                    config_entry.migrated = true;
                    config_entry_changed = true;
                }
            }
        }

        // save the migration map to the config file after releasing the lock in case config_entry_changed
        if config_entry_changed {
            if let Err(e) = self.update_migration_config_file().await {
                error!("Failed to update migration config file: {}", e);
            }
        }
    }
    // load the migration map from the file
    pub fn load_migration_config(config_file_path: &str) -> Option<MigrationMap> {
        let file = File::open(config_file_path).ok()?;
        let reader = std::io::BufReader::new(file);
        Some(serde_json::from_reader(reader).unwrap_or_else(|_| HashMap::new()))
    }

    // function to update the migration status in the config file
    async fn update_migration_config_file(&self) -> Result<(), String> {
        if Path::new(&self.config_file_path).exists() {
            let migration_map = self.migration_config.lock().await;
            let file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&self.config_file_path)
                .map_err(|e| format!("Failed to open migration config file: {}", e))?;
            serde_json::to_writer_pretty(file, &*migration_map)
                .map_err(|e| format!("Failed to write to migration config file: {}", e))?;
            Ok(())
        } else {
            Err(format!(
                "Migration config file not found at path {}",
                self.config_file_path
            ))
        }
    }
}
