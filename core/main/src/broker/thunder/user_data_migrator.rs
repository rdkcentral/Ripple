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
    collections::HashMap,
    fmt,
    fs::{File, OpenOptions},
    path::Path,
    sync::Arc,
};

use ripple_sdk::{
    log::{debug, error, info},
    tokio::{
        self,
        net::TcpStream,
        sync::mpsc::{self, Receiver, Sender},
        sync::Mutex,
        time::{timeout, Duration},
    },
    utils::error::RippleError,
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

// TBD get the storage dir from manifest or other Ripple config file
const RIPPLE_STORAGE_DIR: &str = "/opt/persistent/ripple";
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
pub struct CoversionRule {
    conversion_rule: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MigrationConfigEntry {
    namespace: String,
    key: String,
    default: Value,
    getter: String,
    setter: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    setter_rule: Option<Rule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_to_plugin_value_conversion: Option<CoversionRule>,
    migrated: bool,
}

type MigrationMap = HashMap<String, MigrationConfigEntry>;
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
                "{}/{}",
                RIPPLE_STORAGE_DIR, USER_DATA_MIGRATION_CONFIG_FILE_NAME
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

    /// function to intercept and handle broker request. Perform migration if needed
    pub async fn intercept_broker_request(
        &self,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        request: &mut BrokerRequest,
    ) -> bool {
        let method = request.rpc.method.clone();
        info!(
            "intercept_broker_request: Intercepting broker request for method: {:?}",
            method
        );
        if let Some(config_entry) = self.get_matching_migration_entry_by_method(&method).await {
            if config_entry.setter == method {
                return self
                    .handle_setter_request(broker, ws_tx, request, &config_entry)
                    .await;
            } else {
                return self
                    .handle_getter_request(broker, ws_tx, request, &config_entry)
                    .await;
            }
        }

        // Continue with the original request if no migration entry is found
        false
    }

    async fn handle_setter_request(
        &self,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        request: &BrokerRequest,
        config_entry: &MigrationConfigEntry,
    ) -> bool {
        info!(
            "intercept_broker_request: Handling setter request for method: {:?}",
            config_entry.setter
        );
        self.set_migration_status(&config_entry.namespace, &config_entry.key)
            .await;

        let params = self.extract_params(&request.rpc.params_json);
        info!(
            "intercept_broker_request: Updating legacy storage with new value: {:?}",
            params
        );

        let _ = self
            .write_to_legacy_storage(
                &config_entry.namespace,
                &config_entry.key,
                broker,
                ws_tx.clone(),
                &params,
            )
            .await;

        // Return false to continue with the original setter request
        false
    }

    async fn handle_getter_request(
        &self,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        request: &BrokerRequest,
        config_entry: &MigrationConfigEntry,
    ) -> bool {
        info!(
            "intercept_broker_request: Handling getter request for method: {:?}",
            config_entry.getter
        );
        if !config_entry.migrated {
            let self_arc = Arc::new(self.clone());
            self_arc
                .invoke_perform_getter_migration(broker, ws_tx.clone(), request, config_entry)
                .await;
            return true;
        }

        // The migration already done, continue with the original request
        info!(
            "intercept_broker_request: Migration already done for method: {:?}",
            config_entry.getter
        );
        false
    }

    fn extract_params(&self, params_json: &str) -> Value {
        if let Ok(mut extract) = serde_json::from_str::<Vec<Value>>(params_json) {
            if let Some(last) = extract.pop() {
                return last.get("value").cloned().unwrap_or(last);
            }
        }
        Value::Null
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

        // Register custom callback to handle the response
        broker
            .register_custom_callback(
                request_id,
                BrokerCallback {
                    sender: self.response_tx.clone(),
                },
            )
            .await;

        // create the request to the legacy storage
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

        info!(
            "read_from_legacy_storage: Sending request to plugin: {:?}",
            thunder_request
        );

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
        // get the response and check if the response is successful by checking result or error field.
        // Value::Null is a valid response, return Err if the response is not successful
        let response =
            Self::wait_for_response(self.response_rx.clone(), broker.clone(), request_id).await;
        match response {
            Ok(response) => {
                if let Some(result) = response.data.result {
                    // extract the value field from the result
                    if let Some(value) = result.get("value") {
                        return Ok(value.clone());
                    }
                    Err(UserDataMigratorError::ResponseError(
                        "No value field in response".to_string(),
                    ))
                } else {
                    Err(UserDataMigratorError::ResponseError(
                        "No result in response".to_string(),
                    ))
                }
            }
            Err(e) => Err(e),
        }
    }

    async fn write_to_legacy_storage(
        &self,
        namespace: &str,
        key: &str,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        params_json: &Value,
    ) -> Result<(), UserDataMigratorError> {
        let request_id = EndpointBrokerState::get_next_id();
        let call_sign = "org.rdk.PersistentStore.1.".to_owned();

        // Register custom callback to handle the response
        broker
            .register_custom_callback(
                request_id,
                BrokerCallback {
                    sender: self.response_tx.clone(),
                },
            )
            .await;

        // create the request to the legacy storage
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
        info!(
            "write_to_legacy_storage: Sending request to plugin: {:?}",
            thunder_request
        );

        // send the request to the legacy storage
        if let Err(e) = self.send_thunder_request(&ws_tx, &thunder_request).await {
            error!(
                "write_to_legacy_storage: Failed to send thunder request: {:?}",
                e
            );
            // Unregister the custom callback and return
            broker.unregister_custom_callback(request_id).await;
            return Ok(());
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
        Ok(())
    }

    async fn read_from_thunder_plugin(
        &self,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        request: &BrokerRequest,
    ) -> Result<BrokerOutput, UserDataMigratorError> {
        let request_id = EndpointBrokerState::get_next_id();

        // Register custom callback to handle the response
        broker
            .register_custom_callback(
                request_id,
                BrokerCallback {
                    sender: self.response_tx.clone(),
                },
            )
            .await;

        // Create the request to the new plugin
        // The current implementation assumes no params for the getter function
        // extend the migration configuration to support params if needed
        let thunder_plugin_request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": request.rule.alias,
        })
        .to_string();

        info!(
            "read_from_thunder_plugin: Sending request to plugin: {:?}",
            thunder_plugin_request
        );

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
        Self::wait_for_response(self.response_rx.clone(), broker.clone(), request_id).await
    }

    fn transform_requets_params(
        params_json: &Value,
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

    async fn write_to_thunder_plugin(
        &self,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        config_entry: &MigrationConfigEntry,
        params_json: &Value, // param from the legacy storage
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

        let request_id = EndpointBrokerState::get_next_id();

        // Register custom callback to handle the response
        broker
            .register_custom_callback(
                request_id,
                BrokerCallback {
                    sender: self.response_tx.clone(),
                },
            )
            .await;

        // create the request to the new plugin
        let thunder_plugin_request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": setter_rule.alias,
            "params": transformed_params,
        })
        .to_string();

        info!(
            "write_to_thunder_plugin: Sending request to plugin: {:?}",
            thunder_plugin_request
        );

        // send the request to the new plugin as thunder request
        if let Err(e) = self
            .send_thunder_request(&ws_tx, &thunder_plugin_request)
            .await
        {
            error!(
                "write_to_thunder_plugin: Failed to send thunder request: {:?}",
                e
            );
            broker.unregister_custom_callback(request_id).await;
            return Err(e);
        }

        // get the response from the custom callback
        Self::wait_for_response(self.response_rx.clone(), broker.clone(), request_id).await
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
            Ok(rule)
        } else {
            Err(UserDataMigratorError::SetterRuleNotAvailable)
        }
    }

    async fn invoke_perform_getter_migration(
        self: Arc<Self>,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        request: &BrokerRequest,
        config_entry: &MigrationConfigEntry,
    ) {
        // Clone the parameters to move into the spawned task
        let broker_clone = broker.clone();
        let ws_tx_clone = ws_tx.clone();
        let request_clone = request.clone();
        let config_entry_clone = config_entry.clone();
        let self_clone = Arc::clone(&self);

        tokio::spawn(async move {
            match self_clone
                .perform_getter_migration(
                    &broker_clone,
                    ws_tx_clone.clone(),
                    &request_clone,
                    &config_entry_clone,
                )
                .await
            {
                Ok((_, Some(mut output))) => {
                    // Handle the case where output is returned. Send the response to the default callback of the broker.
                    // The response structure will be upated with the originial request id for the callback handler to match the response.
                    output.data.id = Some(request_clone.rpc.ctx.call_id);
                    if let Err(e) = broker_clone
                        .get_default_callback()
                        .sender
                        .send(output)
                        .await
                    {
                        error!("Failed to send response: {:?}", e);
                    }
                }
                Ok((_, None)) | Err(_) => {
                    // Handle the case where no output is returned. Read the value from the plugin and send the response
                    // The response will be sent to the default callback of the broker.
                    // The response structure will be upated with the originial request id for the callback handler to match the response.
                    let value_from_thunder_plugin = self_clone
                        .read_from_thunder_plugin(
                            &broker_clone,
                            ws_tx_clone.clone(),
                            &request_clone,
                        )
                        .await;
                    match value_from_thunder_plugin {
                        Ok(mut value_from_thunder_plugin) => {
                            value_from_thunder_plugin.data.id = Some(request_clone.rpc.ctx.call_id);
                            if let Err(e) = broker_clone
                                .get_default_callback()
                                .sender
                                .send(value_from_thunder_plugin)
                                .await
                            {
                                error!("Failed to send response: {:?}", e);
                            }
                        }
                        Err(_e) => {
                            broker_clone
                                .get_default_callback()
                                .send_error(request_clone, RippleError::ProcessorError)
                                .await
                        }
                    }
                }
            }
        });
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
            .await?;
        info!(
            "perform_getter_migration: Read from legacy storage: {:?}",
            legacy_value
        );
        let value_from_thunder_plugin = self
            .read_from_thunder_plugin(broker, ws_tx.clone(), request)
            .await?;

        // apply the response transform to the data from the plugin
        let mut data = value_from_thunder_plugin.clone().data;
        if let Some(filter) = request
            .rule
            .transform
            .get_transform_data(RuleTransformType::Response)
        {
            endpoint_broker::apply_response(filter, &request.rule.alias, &mut data);
        }

        if let Some(result) = data.clone().result {
            if result != config_entry.default {
                info!(
                    "perform_getter_migration: Plugin has non-default value. Updating legacy storage with new value: {:?}",
                    result
                );
                self.update_legacy_storage(
                    &config_entry.namespace,
                    &config_entry.key,
                    broker,
                    ws_tx.clone(),
                    &result,
                )
                .await?;
                self.set_migration_status(&config_entry.namespace, &config_entry.key)
                    .await;
                // this BrokerOutput has the data from the plugin
                Ok((true, Some(BrokerOutput { data })))
            } else if legacy_value != config_entry.default {
                info!(
                    "perform_getter_migration: Plugin has default value and Legacy storage has the latest value. Updating plugin with value from legacy storage: {:?}",
                    legacy_value
                );
                let mut response = self
                    .update_plugin_from_legacy(broker, ws_tx.clone(), config_entry, &legacy_value)
                    .await?;
                self.set_migration_status(&config_entry.namespace, &config_entry.key)
                    .await;

                // this BrokerOutput has the data from the legacy storage.
                // prepare the response to send back to the caller as response from plugin but with the data from the legacy storage
                response.data.result = Some(legacy_value.clone());
                // check if there is any value conversion rule available
                if let Some(conversion_rule) = &config_entry.legacy_to_plugin_value_conversion {
                    // apply the conversion rule to the data from the legacy storage
                    let data = crate::broker::rules_engine::jq_compile(
                        json!({ "value": legacy_value }),
                        &conversion_rule.conversion_rule,
                        "legacy_to_plugin_value_conversion".to_string(),
                    );
                    if let Ok(data) = data {
                        response.data.result = Some(data);
                    }
                }
                return Ok((true, Some(response)));
            } else {
                info!(
                    "perform_getter_migration: Both plugin and legacy storage have default value. No migration needed."
                );
                self.set_migration_status(&config_entry.namespace, &config_entry.key)
                    .await;
                return Ok((true, Some(BrokerOutput { data })));
            }
        } else {
            Err(UserDataMigratorError::ResponseError(
                "No result in response".to_string(),
            ))
        }
    }

    async fn update_legacy_storage(
        &self,
        namespace: &str,
        key: &str,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        value: &Value,
    ) -> Result<(), UserDataMigratorError> {
        self.write_to_legacy_storage(namespace, key, broker, ws_tx, value)
            .await
    }

    async fn update_plugin_from_legacy(
        &self,
        broker: &ThunderBroker,
        ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
        config_entry: &MigrationConfigEntry,
        value: &Value,
    ) -> Result<BrokerOutput, UserDataMigratorError> {
        self.write_to_thunder_plugin(broker, ws_tx, config_entry, value)
            .await
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