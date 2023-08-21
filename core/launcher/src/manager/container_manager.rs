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
};

use ripple_sdk::{
    api::{
        apps::{Dimensions, StateChange, ViewId},
        firebolt::fb_lifecycle::LifecycleState,
    },
    log::{debug, error},
};
use serde::{Deserialize, Serialize};

use crate::{
    launcher_state::LauncherState,
    manager::{app_launcher::AppLauncher, container_message::ContainerEvent},
};

use super::{
    container_message::{ContainerError, ResultType},
    stack::Stack,
    view_manager::{Position, ViewManager},
};

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct StateChangeInternal {
    pub states: StateChange,
    pub container_props: ContainerProperties,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ContainerProperties {
    pub name: String,
    pub view_id: ViewId,
    pub requires_focus: bool,
    pub dimensions: Dimensions,
}

#[derive(Debug, Clone, Default)]
pub struct ContainerState {
    stack: Arc<RwLock<Stack>>,
    containers: Arc<RwLock<HashMap<String, ContainerProperties>>>,
}

impl ContainerState {
    fn get_prev_stack(&self) -> Option<String> {
        let prev_container = {
            let stack = self.stack.read().unwrap();
            stack.peek().cloned()
        };
        prev_container
    }

    fn get_container_by_name(&self, id: &String) -> Option<ContainerProperties> {
        {
            let container = self.containers.read().unwrap();
            container.get(id).cloned()
        }
    }

    fn add_container(&self, k: String, v: ContainerProperties) {
        let mut containers = self.containers.write().unwrap();
        containers.insert(k, v);
    }

    fn remove_container(&self, k: String) {
        let mut containers = self.containers.write().unwrap();
        containers.remove(&k);
    }

    fn contains_stack_by_name(&self, id: &String) -> bool {
        let mut stack = self.stack.write().unwrap();
        stack.contains(id)
    }

    fn stack_len(&self) -> usize {
        let stack = self.stack.write().unwrap();
        stack.len()
    }

    fn add_stack(&self, id: String) {
        let mut stack = self.stack.write().unwrap();
        stack.push(id);
    }

    fn pop_stack_by_name(&self, name: &str) {
        let mut stack = self.stack.write().unwrap();
        stack.pop_item(name);
    }

    fn bring_stack_to_front(&self, name: &str) {
        let mut stack = self.stack.write().unwrap();
        stack.bring_to_front(name);
    }

    fn send_stack_to_back(&self, name: &str) {
        let mut stack = self.stack.write().unwrap();
        stack.send_to_back(name);
    }
}

pub struct ContainerManager;

impl ContainerManager {
    pub async fn add(
        state: &LauncherState,
        props: ContainerProperties,
    ) -> Result<ResultType, ContainerError> {
        let name = props.name.clone();
        println!("add: name={}", name);
        let mut prev_props = None;
        let prev_container = state.container_state.get_prev_stack();
        if let Some(pc) = prev_container {
            if !pc.eq(name.as_str()) {
                if let Some(pp) = state.container_state.get_container_by_name(&pc) {
                    prev_props = Some(pp);
                }
            }
        }

        if !state.container_state.contains_stack_by_name(&name) {
            state.container_state.add_stack(name.clone());
        }
        state
            .container_state
            .add_container(name.clone(), props.clone());
        AppLauncher::on_container_event(state, ContainerEvent::Added(props.clone())).await;
        Self::bring_to_front(state, &name).await.ok();
        AppLauncher::on_container_event(state, ContainerEvent::Focused(prev_props, Some(props)))
            .await;
        Self::set_visible(state, &name, true).await
    }

    pub async fn remove(state: &LauncherState, name: &str) -> Result<ResultType, ContainerError> {
        let mut result = Ok(ResultType::None);
        if state
            .container_state
            .contains_stack_by_name(&name.to_string())
        {
            let mut prev_props = None;
            if let Some(pp) = state.container_state.get_container_by_name(&name.into()) {
                prev_props = Some(pp);
            }
            state.container_state.pop_stack_by_name(name);
            let mut next_props = None;
            if let Some(nc) = state.container_state.get_prev_stack() {
                if let Some(np) = state.container_state.get_container_by_name(&nc) {
                    next_props = Some(np);
                }
                let next_container = nc.clone();
                if let Err(e) = Self::bring_to_front(state, &next_container).await {
                    println!("remove: Failed to focus top container: e={:?}", e);
                    result = Err(ContainerError::General);
                }
            }
            AppLauncher::on_container_event(
                state,
                ContainerEvent::Focused(prev_props.clone(), next_props),
            )
            .await;
            if let Some(p) = prev_props {
                AppLauncher::on_container_event(state, ContainerEvent::Removed(p)).await;
            }
        } else {
            result = Err(ContainerError::NotFound);
        }

        state.container_state.remove_container(name.into());

        result
    }

    pub async fn bring_to_front(
        state: &LauncherState,
        name: &str,
    ) -> Result<ResultType, ContainerError> {
        if !state
            .container_state
            .contains_stack_by_name(&name.to_string())
        {
            println!("bring_to_front: Not found in stack: name={}", name);
            return Err(ContainerError::NotFound);
        }

        let item = state.container_state.get_container_by_name(&name.into());
        if item.is_none() {
            println!("bring_to_front: Container not found:  name={}", name);
            return Err(ContainerError::NotFound);
        }

        state.container_state.bring_stack_to_front(name);

        let props = item.unwrap().clone();
        let resp = ViewManager::set_position(state, props.view_id, Position::Front).await;
        if let Err(e) = resp {
            println!("bring_to_front: error: req_id={:?}", e);
            return Err(ContainerError::General);
        }
        match Self::focus_top_container(state).await {
            Ok(v) => Ok(ResultType::Uuid(v)),
            Err(e) => Err(e),
        }
    }

    async fn focus_top_container(state: &LauncherState) -> Result<ViewId, ContainerError> {
        let item = state.container_state.get_prev_stack();

        if item.is_none() {
            return Err(ContainerError::NotFound);
        }

        let top_container = item.unwrap();
        let p = state
            .container_state
            .get_container_by_name(&top_container)
            .unwrap();
        let properties = p.clone();
        let view_id = properties.view_id;
        match ViewManager::set_focus(state, view_id).await {
            Ok(v) => Ok(v),
            Err(_e) => Err(ContainerError::General),
        }
    }

    pub async fn send_to_back(
        state: &LauncherState,
        name: &str,
    ) -> Result<ResultType, ContainerError> {
        if !state
            .container_state
            .contains_stack_by_name(&name.to_string())
        {
            error!("send_to_back: Not found in stack: name={}", name);
            return Err(ContainerError::NotFound);
        }

        let stack_size = state.container_state.stack_len();
        if stack_size < 2 {
            error!("send_to_back: Not enough containers {}", stack_size);
            return Err(ContainerError::General);
        }

        let item = state.container_state.get_container_by_name(&name.into());

        if item.is_none() {
            error!("send_to_back: Container not found:  name={}", name);
            return Err(ContainerError::NotFound);
        }

        let props = item.unwrap().clone();
        state.container_state.send_stack_to_back(name);
        let view_id = props.view_id;
        let mut result = Ok(ResultType::None);
        let resp = ViewManager::set_position(state, view_id, Position::Back).await;
        if resp.is_err() {
            error!("send_to_back: error: req_id={:?}", resp);
            result = Err(ContainerError::General);
        }

        Self::focus_top_container(state).await.ok();
        let mut next_props = None;
        let name = state.container_state.get_prev_stack().unwrap();
        if let Some(n) = state.container_state.get_container_by_name(&name) {
            next_props = Some(n);
        }
        AppLauncher::on_container_event(state, ContainerEvent::Focused(Some(props), next_props))
            .await;
        result
    }

    async fn set_visible(
        state: &LauncherState,
        name: &str,
        visible: bool,
    ) -> Result<ResultType, ContainerError> {
        if !state
            .container_state
            .contains_stack_by_name(&name.to_string())
        {
            println!("set_visible: Not found in stack: name={}", name);
            return Err(ContainerError::NotFound);
        }

        let item = state.container_state.get_container_by_name(&name.into());
        if item.is_none() {
            println!("set_visible: Container not found:  name={}", name);
            return Err(ContainerError::NotFound);
        }

        let props = item.unwrap();
        let view_id = props.view_id;
        let resp = ViewManager::set_visibility(state, view_id, visible).await;
        match resp {
            Ok(_) => Ok(ResultType::None),
            Err(_) => {
                error!("set_visible: error: req_id={:?}", resp);
                Err(ContainerError::General)
            }
        }
    }

    pub async fn on_state_changed(state: &LauncherState, state_change: StateChangeInternal) {
        debug!("on_app_state_change: state_change={:?}", state_change);
        let app_id = state_change.container_props.name.clone();
        if (state_change.states.previous != LifecycleState::Initializing
            && state_change.states.state == LifecycleState::Inactive)
            || state_change.states.state == LifecycleState::Unloading
        {
            let _ = Self::remove(state, &app_id).await;
        }
    }
}
