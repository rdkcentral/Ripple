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
use ripple_sdk::{
    tokio::sync::{mpsc, Mutex},
    tokio_tungstenite::tungstenite::Message,
    utils::error::RippleError,
};
use std::collections::HashMap;

use super::service_controller_state::ServiceInfo;
use crate::broker::endpoint_broker::BrokerCallback;
#[derive(Debug, Default)]
pub struct ServiceRegistry {
    service_registry: Mutex<HashMap<String, ServiceInfo>>,
}

impl ServiceRegistry {
    pub async fn add_service_info(
        &self,
        service_id: String,
        info: ServiceInfo,
    ) -> Result<(), RippleError> {
        let old_tx = {
            let mut registry = self.service_registry.lock().await;
            let old_tx = registry.get(&service_id).map(|info| info.tx.clone());
            // insert the new service info
            registry.insert(service_id, info);
            old_tx
        };
        // Now, outside the lock, optionally send a disconnect message to the old client
        if let Some(tx) = old_tx {
            let _ = tx.send(Message::Close(None)).await;
        }
        Ok(())
    }

    pub async fn remove_service_info(&self, service_id: &String) -> Result<(), RippleError> {
        let mut registry = self.service_registry.lock().await;
        if registry.remove(service_id).is_some() {
            Ok(())
        } else {
            Err(RippleError::InvalidInput)
        }
    }

    // get sender for a given service_id
    pub async fn get_sender(&self, service_id: &String) -> Option<mpsc::Sender<Message>> {
        let registry = self.service_registry.lock().await;
        registry.get(service_id).map(|info| info.tx.clone())
    }

    // set Broker callback for a given service_id
    pub async fn set_broker_callback(
        &self,
        service_id: &String,
        request_id: u64,
        callback: BrokerCallback,
    ) -> Result<(), RippleError> {
        let mut registry = self.service_registry.lock().await;
        if let Some(info) = registry.get_mut(service_id) {
            info.add_callback(request_id, callback).await;
            Ok(())
        } else {
            Err(RippleError::InvalidInput)
        }
    }

    // get the broker callback for a given service_id
    pub async fn extract_broker_callback(
        &self,
        service_id: &String,
        request_id: u64,
    ) -> Result<Option<BrokerCallback>, RippleError> {
        let mut registry = self.service_registry.lock().await;
        if let Some(info) = registry.get_mut(service_id) {
            Ok(info.get_and_remove_callback(request_id).await)
        } else {
            Err(RippleError::InvalidInput)
        }
    }
}
