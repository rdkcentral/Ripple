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

use std::{collections::HashMap, sync::Arc};

use ripple_sdk::{
    api::{
        apps::{Dimensions, ViewId},
        device::{
            device_browser::{BrowserDestroyParams, BrowserLaunchParams, BrowserRequest},
            device_window_manager::WindowManagerRequest,
        },
        manifest::apps::AppProperties,
    },
    log::error,
    parking_lot::RwLock,
    tokio::sync::oneshot,
    utils::{channel_utils::oneshot_send_and_log, error::RippleError},
    uuid::Uuid,
};
use serde::{Deserialize, Serialize};

use crate::launcher_state::LauncherState;

#[derive(Debug, Clone)]
pub enum ViewMethod {
    Acquire(LaunchParams),
    Release(ViewId),
    SetPosition(ViewId, Position),
    SetFocus(ViewId),
    SetVisibility(ViewId, bool),
    SetDimensions(ViewId, Dimensions),
}

#[derive(Debug, Clone)]
pub enum Position {
    Front,
    Back,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LaunchParams {
    // TODO: LaunchParams is a temporary struct until app manifest support is added. For now this simply
    // reflects the JSON contents of a file in order to demonstrate app launches.
    pub uri: String,
    pub browser_name: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub suspend: bool,
    pub requires_focus: bool,
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub properties: Option<AppProperties>,
}

#[derive(Debug, Clone)]
pub struct ViewRequest {
    pub method: ViewMethod,
    pub resp_tx: Arc<RwLock<Option<oneshot::Sender<ViewResponse>>>>,
}

impl ViewRequest {
    pub fn new(method: ViewMethod, sender: oneshot::Sender<ViewResponse>) -> ViewRequest {
        ViewRequest {
            method,
            resp_tx: Arc::new(RwLock::new(Some(sender))),
        }
    }

    pub fn send_response(&self, response: ViewResponse) -> Result<(), RippleError> {
        let mut sender = self.resp_tx.write();
        match sender.take() {
            Some(tx) => {
                oneshot_send_and_log(tx, response, "ViewManager response");
                Ok(())
            }
            None => Err(RippleError::SenderMissing),
        }
    }
}

#[derive(Debug)]
pub struct ViewResponse {
    pub req_id: u64,
    pub result: Result<ViewId, ViewError>,
}

#[derive(Debug)]
pub enum ViewError {
    General,
    NotFound,
    NotSupported,
    IoError,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum Fnci {
    Web,
    Lightning,
    Netflix,
    Cobalt,
    PrimeVideo,
    DisneyPlus,
}

pub fn get_fnci(runtime: &str) -> Option<Fnci> {
    match runtime {
        "web" => Some(Fnci::Web),
        "lightning" => Some(Fnci::Lightning),
        "netflix" => Some(Fnci::Netflix),
        "cobalt" => Some(Fnci::Cobalt),
        "primevideo" => Some(Fnci::PrimeVideo),
        "disneyplus" => Some(Fnci::DisneyPlus),
        _ => None,
    }
}

pub struct ViewManager;

#[derive(Debug, Clone, Default)]
pub struct ViewState {
    view_pool: Arc<RwLock<HashMap<String, ViewId>>>,
}

impl ViewState {
    fn insert_view(&self, key: String, view: ViewId) {
        let _ = self.view_pool.write().insert(key, view);
    }

    fn get_name(&self, key: ViewId) -> Option<String> {
        self.view_pool.read().iter().find_map(
            |(name, &id)| {
                if id == key {
                    Some(name.clone())
                } else {
                    None
                }
            },
        )
    }

    fn remove(&self, key: &str) {
        let _ = self.view_pool.write().remove(key);
    }
}

impl ViewManager {
    pub async fn acquire_view(
        state: &LauncherState,
        params: LaunchParams,
    ) -> Result<ViewId, ViewError> {
        let view_id = Uuid::new_v4();
        let dab_resp = state
            .send_extn_request(BrowserRequest::Start(BrowserLaunchParams {
                uri: params.uri,
                browser_name: params.browser_name.clone(),
                _type: params._type,
                visible: false,
                suspend: params.suspend,
                focused: params.requires_focus,
                name: params.name,
                x: params.x,
                y: params.y,
                w: params.w,
                h: params.h,
                properties: params.properties.map(|r| r.get_browser_props()),
            }))
            .await;

        match dab_resp {
            Ok(_resp) => {
                state.view_state.insert_view(params.browser_name, view_id);
                Ok(view_id)
            }
            Err(_e) => Err(ViewError::General),
        }
    }

    pub async fn release_view(state: &LauncherState, id: ViewId) -> Result<ViewId, ViewError> {
        let mut result = Err(ViewError::NotFound);
        if let Some(name) = state.view_state.get_name(id) {
            let dab_resp = state
                .send_extn_request(BrowserRequest::Destroy(BrowserDestroyParams {
                    browser_name: name.clone(),
                }))
                .await;

            if let Err(e) = dab_resp {
                error!("release_view: Error destroying view: e={:?}", e);
            }

            state.view_state.remove(&name);
            result = Ok(id);
        }
        result
    }

    pub async fn set_position(
        state: &LauncherState,
        id: ViewId,
        position: Position,
    ) -> Result<ViewId, ViewError> {
        let result;
        match state.view_state.get_name(id) {
            Some(name) => {
                let method = match position {
                    Position::Front => WindowManagerRequest::MoveToFront(name),
                    Position::Back => WindowManagerRequest::MoveToBack(name),
                };

                let dab_resp = state.send_extn_request(method).await;

                match dab_resp {
                    Ok(_resp) => {
                        result = Ok(id);
                    }
                    Err(_e) => {
                        result = Err(ViewError::General);
                    }
                }

                if Self::set_visibility(state, id, matches!(position, Position::Front))
                    .await
                    .is_err()
                {
                    error!("Couldnt set visibility for view id {:?}", id)
                }
            }
            None => {
                result = Err(ViewError::NotFound);
            }
        }
        result
    }

    pub async fn set_focus(state: &LauncherState, id: ViewId) -> Result<ViewId, ViewError> {
        let result;
        match state.view_state.get_name(id) {
            Some(name) => {
                let dab_resp = state
                    .send_extn_request(WindowManagerRequest::Focus(name))
                    .await;

                match dab_resp {
                    Ok(_resp) => {
                        result = Ok(id);
                    }
                    Err(_e) => {
                        result = Err(ViewError::General);
                    }
                }
            }
            None => {
                result = Err(ViewError::NotFound);
            }
        }
        result
    }

    pub async fn set_visibility(
        state: &LauncherState,
        id: ViewId,
        visible: bool,
    ) -> Result<ViewId, ViewError> {
        let result;
        match state.view_state.get_name(id) {
            Some(name) => {
                let dab_resp = state
                    .send_extn_request(WindowManagerRequest::Visibility(name, visible))
                    .await;

                match dab_resp {
                    Ok(_resp) => {
                        result = Ok(id);
                    }
                    Err(_e) => {
                        result = Err(ViewError::General);
                    }
                }
            }
            None => {
                result = Err(ViewError::NotFound);
            }
        }
        result
    }
}
