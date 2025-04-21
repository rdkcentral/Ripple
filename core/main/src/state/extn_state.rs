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
    sync::{Arc, RwLock},
    thread,
};

use jsonrpsee::core::server::rpc_module::Methods;
use ripple_sdk::{
    api::{
        manifest::extn_manifest::{ExtnManifest, ExtnManifestEntry, ExtnSymbol},
        status_update::ExtnStatus,
    },
    extn::{
        extn_id::ExtnId,
        ffi::{ffi_channel::ExtnChannel, ffi_library::ExtnMetadata},
    },
    libloading::Library,
    log::info,
    tokio::sync::mpsc,
    utils::error::RippleError,
};

use crate::service::extn::ripple_client::RippleClient;

use super::bootstrap_state::ChannelsState;

#[derive(Debug)]
pub struct LoadedLibrary {
    pub library: Library,
    pub metadata: Box<ExtnMetadata>,
    pub entry: ExtnManifestEntry,
}

impl LoadedLibrary {
    pub fn new(
        library: Library,
        metadata: Box<ExtnMetadata>,
        entry: ExtnManifestEntry,
    ) -> LoadedLibrary {
        LoadedLibrary {
            library,
            metadata,
            entry,
        }
    }

    pub fn get_channels(&self) -> Vec<ExtnSymbol> {
        let extn_ids: Vec<String> = self
            .metadata
            .symbols
            .iter()
            .filter(|x| x.id.is_channel())
            .map(|x| x.id.to_string())
            .collect();
        info!(
            "getting channels {} lib_metadata={:?} extn_metadata={:?}",
            self.metadata.symbols.len(),
            extn_ids,
            self.entry
        );
        self.entry
            .symbols
            .iter()
            .filter(|x| extn_ids.contains(&x.id))
            .cloned()
            .collect()
    }

    pub fn get_extns(&self) -> Vec<ExtnSymbol> {
        let extn_ids: Vec<String> = self
            .metadata
            .symbols
            .iter()
            .filter(|x| x.id.is_extn())
            .map(|x| x.id.to_string())
            .collect();
        self.entry
            .symbols
            .iter()
            .filter(|x| extn_ids.contains(&x.id))
            .cloned()
            .collect()
    }

    pub fn get_symbols(&self) {}

    pub fn get_metadata(&self) -> Box<ExtnMetadata> {
        self.metadata.clone()
    }
}

#[derive(Debug)]
pub struct PreLoadedExtnChannel {
    pub channel: Box<ExtnChannel>,
    pub extn_id: ExtnId,
    pub symbol: ExtnSymbol,
}

/// Bootstrap state which is used to store transient extension information used while bootstrapping.
/// Content within state is related to extension symbols and Libraries.
#[derive(Debug, Clone)]
pub struct ExtnState {
    pub permission_map: HashMap<String, Vec<String>>,
    pub loaded_libraries: Arc<RwLock<Vec<LoadedLibrary>>>,
    pub device_channels: Arc<RwLock<Vec<PreLoadedExtnChannel>>>,
    pub deferred_channels: Arc<RwLock<Vec<PreLoadedExtnChannel>>>,
    extn_status_map: Arc<RwLock<HashMap<String, ExtnStatus>>>,
    extn_status_listeners: Arc<RwLock<HashMap<String, mpsc::Sender<ExtnStatus>>>>,
    pub extn_methods: Arc<RwLock<Methods>>,
}

impl ExtnState {
    pub fn new(manifest: ExtnManifest) -> ExtnState {
        ExtnState {
            permission_map: manifest.get_extn_permissions(),
            loaded_libraries: Arc::new(RwLock::new(Vec::new())),
            device_channels: Arc::new(RwLock::new(Vec::new())),
            deferred_channels: Arc::new(RwLock::new(Vec::new())),
            extn_status_map: Arc::new(RwLock::new(HashMap::new())),
            extn_status_listeners: Arc::new(RwLock::new(HashMap::new())),
            extn_methods: Arc::new(RwLock::new(Methods::new())),
        }
    }

    pub fn update_extn_status(&self, id: ExtnId, status: ExtnStatus) {
        let mut extn_status_map = self.extn_status_map.write().unwrap();
        let _ = extn_status_map.insert(id.to_string(), status);
    }

    pub fn is_extn_ready(&self, extn_id: ExtnId) -> bool {
        if let Some(ExtnStatus::Ready) = self
            .extn_status_map
            .read()
            .unwrap()
            .get(extn_id.to_string().as_str())
        {
            return true;
        }
        false
    }

    pub fn add_extn_status_listener(&self, id: ExtnId, sender: mpsc::Sender<ExtnStatus>) -> bool {
        {
            if self.is_extn_ready(id.clone()) {
                return true;
            }
        }
        let mut extn_status_listeners = self.extn_status_listeners.write().unwrap();
        let _ = extn_status_listeners.insert(id.to_string(), sender);
        false
    }

    pub fn get_extn_status_listener(&self, id: ExtnId) -> Option<mpsc::Sender<ExtnStatus>> {
        let extn_status_listeners = self.extn_status_listeners.read().unwrap();
        extn_status_listeners.get(id.to_string().as_str()).cloned()
    }

    pub fn clear_status_listener(&self, extn_id: ExtnId) {
        let mut extn_status_listeners = self.extn_status_listeners.write().unwrap();
        let _ = extn_status_listeners.remove(extn_id.to_string().as_str());
    }


    pub fn start_channel(
        &mut self,
        channel: PreLoadedExtnChannel,
    ) -> Result<(), RippleError> {
        let symbol = channel.symbol.clone();
        let extn_id = channel.extn_id.clone();
        
        let extn_channel = channel.channel;
        thread::spawn(move || {
            (extn_channel.start)();
        });
        Ok(())
    }

    pub fn extend_methods(&self, methods: Methods) {
        let mut methods_state = self.extn_methods.write().unwrap();
        let _ = methods_state.merge(methods);
    }

    pub fn get_extn_methods(&self) -> Methods {
        self.extn_methods.read().unwrap().clone()
    }
}
