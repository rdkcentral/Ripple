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
use std::fmt;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Arc;

use jsonrpsee::types::params;
use ripple_sdk::tokio::net::TcpStream;
use ripple_sdk::utils::error::RippleError;
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
    self, BrokerCallback, BrokerOutput, BrokerRequest, EndpointBrokerState,
};
use crate::broker::rules_engine::{Rule, RuleTransformType};

use futures::stream::SplitSink;
use futures_util::SinkExt;

use crate::broker::thunder_broker::ThunderBroker;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
const USER_DATA_MIGRATION_CONFIG_FILE_NAME: &str = "user_data_migration_config.json";

#[derive(Debug)]
enum UserDataMigratorError {
    ThunderRequestError(String),
    ResponseError(String),
    SetterRuleNotAvailable,
    RequestTransformError(String),
    TimeoutError,
}

impl fmt::Display for UserDataMigratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserDataMigratorError::ThunderRequestError(msg) => {
                write!(f, "Thunder request error: {}", msg)
            }
            UserDataMigratorError::ResponseError(msg) => write!(f, "Response error: {}", msg),
            UserDataMigratorError::TimeoutError => write!(f, "Timeout error"),
            UserDataMigratorError::SetterRuleNotAvailable => {
                write!(f, "Setter rule is not available")
            }
            UserDataMigratorError::RequestTransformError(msg) => {
                write!(f, "Request transform error: {}", msg)
            }
        }
    }
}
impl std::error::Error for UserDataMigratorError {}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MigrationConfigEntry {
    namespace: String,
    key: String,
    default: Value,
    getter: String,
    setter_rule: Option<Rule>,
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

    async fn get_matching_migration_entry_by_method(
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
    ) -> (bool, Option<BrokerOutput>) {
        let method = request.rpc.method.clone();
        if let Some(config_entry) = self.get_matching_migration_entry_by_method(&method).await {
            // migration entry found for either getter or setter method
            // for setter case, irrespective of the migration status, update the new value in the new storage and sync
            // with the legacy storage

            if config_entry.setter == method {
                // perform the setter update and sync up logic asynchronously
                // update legacy storage with the new value as fire and forget operation
                self.set_migration_status(&config_entry.namespace, &config_entry.key)
                    .await;

                let mut params = Value::Null;

                // Get the params from the request as is. No need to transform the params for legacy storage
                if let Ok(mut extract) =
                    serde_json::from_str::<Vec<Value>>(&request.rpc.params_json)
                {
                    // Get the param to use in write to legacy storage
                    if let Some(last) = extract.pop() {
                        // Extract the value field from the last
                        params = last.get("value").cloned().unwrap_or(last);
                    }
                }

                info!(
                    "intercept_broker_request: Updating legacy storage with new value: {:?}",
                    params
                );
                self.write_to_legacy_storage(
                    &config_entry.namespace,
                    &config_entry.key,
                    &broker,
                    ws_tx.clone(),
                    params.to_string().as_str(),
                )
                .await;
                // returning false to continue with the original setter request
                return (false, None);
            } else {
                // perform the getter migration logic asynchronously
                if !config_entry.migrated {
                    let migrated_value = self
                        .perform_getter_migration(&broker, ws_tx.clone(), &request, &config_entry)
                        .await;
                    match migrated_value {
                        Ok((status, value)) => {
                            return (status, value);
                        }
                        Err(e) => {
                            error!("Error performing getter migration and continuing without migration {:?}", e);
                            // return false to continue with the original request
                            return (false, None);
                        }
                    }
                } else {
                    // the migration is already done, continue with the original request
                    return (false, None);
                }
            }
        }

        // continue with the original request
        (false, None)
    }

    async fn read_from_legacy_storage(
        &self,
        namespace: &str,
        key: &str,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
    ) -> Result<Value, UserDataMigratorError> {
        let request_id = EndpointBrokerState::get_next_id();
        let call_sign = "org.rdk.PersistentStore.1.".to_owned();
        let thunder_request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": format!("{}getValue", call_sign),
            "params": json!({
                "namespace": namespace,
                "key": key,
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
            error!(
                "read_from_legacy_storage: Failed to send thunder request: {:?}",
                e
            );
            // Unregister the custom callback and return
            broker.unregister_custom_callback(request_id).await;
            return Err(e);
        }
        // get the response from the custom callback
        let response_rx = self.response_rx.clone();
        let broker_clone = broker.clone();
        // get the response and check if the response is successful by checking result or error field.
        // Value::Null is a valid response, return Err if the response is not successful
        let response =
            UserDataMigrator::wait_for_response(response_rx, broker_clone, request_id).await;
        match response {
            Ok(response) => {
                if let Some(result) = response.data.result {
                    return Ok(result);
                } else {
                    return Err(UserDataMigratorError::ResponseError(
                        "No result in response".to_string(),
                    ));
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    async fn write_to_legacy_storage(
        &self,
        namespace: &str,
        key: &str,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        params_json: &str,
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
                "value": params_json,
                "scope": "device",
            })
        })
        .to_string();
        println!(
            "write_to_legacy_storage: thunder_request: {:?}",
            thunder_request
        );
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
            error!(
                "write_to_legacy_storage: Failed to send thunder request: {:?}",
                e
            );
            // Unregister the custom callback and return
            broker.unregister_custom_callback(request_id).await;
            return;
        }

        // Spawn a task to wait for the response as we don't want to block the main thread
        let response_rx = self.response_rx.clone();
        let broker_clone = broker.clone();
        tokio::spawn(async move {
            match UserDataMigrator::wait_for_response(response_rx, broker_clone, request_id).await {
                Ok(response) => {
                    // Handle the successful response here
                    info!(
                        "write_to_legacy_storage: Successfully received response: {:?}",
                        response
                    );
                }
                Err(e) => {
                    error!("Error waiting for response: {:?}", e);
                }
            }
        });
    }

    async fn read_from_true_north_plugin(
        &self,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        request: &BrokerRequest,
    ) -> Result<BrokerOutput, UserDataMigratorError> {
        // no params for the getter function
        let request_id = EndpointBrokerState::get_next_id();
        let thunder_plugin_request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": request.rule.alias,
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

        // send the request to the new pluin as thunder request
        if let Err(e) = self
            .send_thunder_request(&ws_tx, &thunder_plugin_request)
            .await
        {
            error!(
                "perform_getter_migration: Failed to send thunder request: {:?}",
                e
            );
            broker.unregister_custom_callback(request_id).await;
            return Err(e);
        }

        // get the response from the custom callback
        let response_rx = self.response_rx.clone();
        let broker_clone = broker.clone();

        UserDataMigrator::wait_for_response(response_rx, broker_clone, request_id).await
    }

    fn transform_requets_params(
        params_json: &str,
        rule: &Rule,
        method: &str,
    ) -> Result<Value, RippleError> {
        let data: Value = json!({
            "value": params_json
        });

        if let Some(filter) = rule
            .transform
            .get_transform_data(RuleTransformType::Request)
        {
            return crate::broker::rules_engine::jq_compile(
                data,
                &filter,
                format!("{}_request", method),
            );
        }
        Ok(serde_json::to_value(&data).unwrap())
    }
    async fn write_to_true_north_plugin(
        &self,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        config_entry: &MigrationConfigEntry,
        params_json: &str, // param from the legacy storage
    ) -> Result<BrokerOutput, UserDataMigratorError> {
        // get the setter rule from the rule engine by giving the setter method name
        let setter_rule = Self::retrive_setter_rule_from_rule_engine(config_entry)?;
        // apply the setter rule to the params_json
        let transformed_params =
            Self::transform_requets_params(params_json, &setter_rule, &config_entry.setter);
        // rerurn error if the transform fails
        let transformed_params = match transformed_params {
            Ok(params) => params,
            Err(e) => {
                return Err(UserDataMigratorError::RequestTransformError(e.to_string()));
            }
        };
        // create the request to the new plugin
        let request_id = EndpointBrokerState::get_next_id();
        let thunder_plugin_request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": setter_rule.alias,
            "params": transformed_params,
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

        // send the request to the new plugin as thunder request
        if let Err(e) = self
            .send_thunder_request(&ws_tx, &thunder_plugin_request)
            .await
        {
            error!(
                "write_to_true_north_plugin: Failed to send thunder request: {:?}",
                e
            );
            broker.unregister_custom_callback(request_id).await;
            return Err(e);
        }

        // get the response from the custom callback
        let response_rx = self.response_rx.clone();
        let broker_clone = broker.clone();

        UserDataMigrator::wait_for_response(response_rx, broker_clone, request_id).await
    }

    async fn send_thunder_request(
        &self,
        ws_tx: &Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        request: &str,
    ) -> Result<(), UserDataMigratorError> {
        let mut ws_tx = ws_tx.lock().await;
        ws_tx
            .feed(Message::Text(request.to_string()))
            .await
            .map_err(|e| UserDataMigratorError::ThunderRequestError(e.to_string()))?;
        ws_tx
            .flush()
            .await
            .map_err(|e| UserDataMigratorError::ThunderRequestError(e.to_string()))?;
        Ok(())
    }

    async fn wait_for_response(
        response_rx: Arc<Mutex<tokio::sync::mpsc::Receiver<BrokerOutput>>>,
        broker: ThunderBroker,
        request_id: u64,
    ) -> Result<BrokerOutput, UserDataMigratorError> {
        let mut response_rx = response_rx.lock().await;
        let response = match timeout(Duration::from_secs(30), response_rx.recv()).await {
            Ok(Some(response)) => {
                info!("wait_for_response : Received response : {:?}", response);
                response
            }
            Ok(None) => {
                error!("No response received at custom write_to_legacy_storage");
                return Err(UserDataMigratorError::TimeoutError);
            }
            Err(_) => {
                error!("Error receiving response at custom write_to_legacy_storage");
                return Err(UserDataMigratorError::TimeoutError);
            }
        };
        broker.unregister_custom_callback(request_id).await;
        Ok(response)
    }

    fn retrive_setter_rule_from_rule_engine(
        config_entry: &MigrationConfigEntry,
    ) -> Result<Rule, UserDataMigratorError> {
        // TBD: get the getter rule from the rule engine by giving the setter method name
        let setter_rule = config_entry.setter_rule.clone();
        // return rule if available else return error
        if let Some(rule) = setter_rule {
            return Ok(rule);
        } else {
            return Err(UserDataMigratorError::SetterRuleNotAvailable);
        }
    }

    async fn perform_getter_migration(
        &self,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        request: &BrokerRequest,
        config_entry: &MigrationConfigEntry,
    ) -> Result<(bool, Option<BrokerOutput>), UserDataMigratorError> {
        let legacy_value = self
            .read_from_legacy_storage(
                &config_entry.namespace,
                &config_entry.key,
                broker,
                ws_tx.clone(),
            )
            .await;

        match legacy_value {
            Ok(legacy_value) => {
                let value_from_plugin = self
                    .read_from_true_north_plugin(broker, ws_tx.clone(), request)
                    .await;
                match value_from_plugin {
                    Ok(value_from_plugin) => {
                        // apply the response transform rule if any
                        let mut data = value_from_plugin.clone().data;
                        if let Some(filter) = request
                            .rule
                            .transform
                            .get_transform_data(RuleTransformType::Response)
                        {
                            endpoint_broker::apply_response(filter, &request.rule.alias, &mut data);
                        }
                        if let Some(result) = data.clone().result {
                            // if the plugins has a non default value, assuming that it is holding the latest value
                            // update the legacy storage with the new value
                            if result != config_entry.default {
                                self.write_to_legacy_storage(
                                    &config_entry.namespace,
                                    &config_entry.key,
                                    broker,
                                    ws_tx.clone(),
                                    &result.to_string(),
                                )
                                .await;
                                self.set_migration_status(
                                    &config_entry.namespace,
                                    &config_entry.key,
                                )
                                .await;

                                // create broker output with the result
                                let response = BrokerOutput { data };
                                return Ok((true, Some(response)));
                            } else {
                                // plugin has the default value, now check if the legacy storage has a non default value
                                // if so, update the plugin with the value from the legacy storage
                                if legacy_value != config_entry.default {
                                    let response = self
                                        .write_to_true_north_plugin(
                                            broker,
                                            ws_tx.clone(),
                                            config_entry,
                                            &legacy_value.to_string(),
                                        )
                                        .await;
                                    match response {
                                        Ok(response) => {
                                            self.set_migration_status(
                                                &config_entry.namespace,
                                                &config_entry.key,
                                            )
                                            .await;
                                            return Ok((true, Some(response)));
                                        }
                                        Err(e) => {
                                            return Err(e);
                                        }
                                    }
                                } else {
                                    // both the plugin and the legacy storage has the default value, no need to update
                                    // continue with the original request
                                    self.set_migration_status(
                                        &config_entry.namespace,
                                        &config_entry.key,
                                    )
                                    .await;
                                    // create broker output with the result
                                    let response = BrokerOutput { data };
                                    return Ok((true, Some(response)));
                                }
                            }
                        } else {
                            return Err(UserDataMigratorError::ResponseError(
                                "No result in response".to_string(),
                            ));
                        }
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
            Err(e) => {
                // TBD : Add more detailed error code like no entry in legacy storage, etc
                return Err(e);
            }
        }
    }

    // function to set the migration flag to true and update the migration map in the config file
    async fn set_migration_status(&self, namespace: &str, key: &str) {
        let mut config_entry_changed = false;
        {
            let mut migration_map = self.migration_config.lock().await;
            if let Some(config_entry) = migration_map
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
