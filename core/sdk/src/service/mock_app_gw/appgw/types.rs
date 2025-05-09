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
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

// This mock modules should be able to build and run independently
// Do not use any data structures or modules from the main codebase.
// This is a mock implementation of the App Gateway for testing purposes
pub type Tx = mpsc::Sender<tokio_tungstenite::tungstenite::Message>;
pub type Clients = std::sync::Arc<tokio::sync::Mutex<HashMap<String, ClientInfo>>>;
pub type PendingMap = std::sync::Arc<tokio::sync::Mutex<HashMap<u64, PendingAggregate>>>;
pub type RoutedMap = std::sync::Arc<tokio::sync::Mutex<HashMap<u64, RoutedRequest>>>;

#[derive(Clone, Debug)]
pub struct ClientInfo {
    pub id: String,
    pub tx: Tx,
    pub is_service: bool,
    pub service_id: Option<String>,
}

#[derive(Debug)]
pub struct PendingAggregate {
    pub original_id: u64,
    pub sender_tx: Tx,
    pub expected: HashSet<String>,
    pub responses: HashMap<String, Value>,
}

pub struct RoutedRequest {
    pub service_id: String,
    pub original_id: u64,
    pub sender_tx: Tx,
}
