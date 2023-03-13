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
            let r = stack.peek().clone();
            if r.is_none() {
                None
            } else {
                let v = r.unwrap().clone();
                Some(v)
            }
        };
        prev_container
    }

    fn get_container_by_name(&self, id: &String) -> Option<ContainerProperties> {
        {
            let container = self.containers.read().unwrap();
            let r = container.get(id);
            if r.is_none() {
                None
            } else {
                let v = r.unwrap().clone();
                Some(v)
            }
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
        let mut stack = self.stack.write().unwrap();
        stack.len()
    }

    fn add_stack(&self, id: String) {
        let mut stack = self.stack.write().unwrap();
        stack.push(id);
    }

    fn pop_stack_by_name(&self, name: &String) {
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
                    prev_props = Some(pp.clone());
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
        Self::bring_to_front(&state, &name).await.ok();
        AppLauncher::on_container_event(state, ContainerEvent::Focused(prev_props, Some(props)))
            .await;
        Self::set_visible(&state, &name, true).await
    }

    pub async fn remove(state: &LauncherState, name: &str) -> Result<ResultType, ContainerError> {
        let mut result = Ok(ResultType::None);
        if state
            .container_state
            .contains_stack_by_name(&name.to_string())
        {
            let mut prev_props = None;
            if let Some(pp) = state.container_state.get_container_by_name(&name.into()) {
                prev_props = Some(pp.clone());
            }
            state.container_state.pop_stack_by_name(&name.into());
            let mut next_props = None;
            if let Some(nc) = state.container_state.get_prev_stack() {
                if let Some(np) = state.container_state.get_container_by_name(&nc) {
                    next_props = Some(np.clone());
                }
                let next_container = nc.clone();
                if let Err(e) = Self::bring_to_front(&state, &next_container).await {
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

    async fn bring_to_front(
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
        if let None = item {
            println!("bring_to_front: Container not found:  name={}", name);
            return Err(ContainerError::NotFound);
        }

        state.container_state.bring_stack_to_front(name);

        let props = item.unwrap().clone();
        let resp = ViewManager::set_position(&state, props.view_id, Position::Front).await;
        if let Err(e) = resp {
            println!("bring_to_front: error: req_id={:?}", e);
            return Err(ContainerError::General);
        }
        match Self::focus_top_container(&state).await {
            Ok(v) => Ok(ResultType::Uuid(v)),
            Err(e) => Err(e),
        }
    }

    async fn focus_top_container(state: &LauncherState) -> Result<ViewId, ContainerError> {
        let item = state.container_state.get_prev_stack();

        if let None = item {
            return Err(ContainerError::NotFound);
        }

        let top_container = item.unwrap();
        let p = state
            .container_state
            .get_container_by_name(&top_container)
            .unwrap();
        let properties = p.clone();
        let view_id = properties.view_id.clone();
        match ViewManager::set_focus(&state, view_id).await {
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
            println!("send_to_back: Not found in stack: name={}", name);
            return Err(ContainerError::NotFound);
        }

        let stack_size = state.container_state.stack_len();
        if stack_size < 2 {
            println!("send_to_back: Not enough containers {}", stack_size);
            return Err(ContainerError::General);
        }

        let item = state.container_state.get_container_by_name(&name.into());

        if let None = item {
            println!("send_to_back: Container not found:  name={}", name);
            return Err(ContainerError::NotFound);
        }

        let props = item.unwrap().clone();
        state.container_state.send_stack_to_back(name);
        let view_id = props.view_id.clone();
        let mut result = Ok(ResultType::None);
        let resp = ViewManager::set_position(&state, view_id, Position::Back).await;
        if let Err(_) = resp {
            println!("send_to_back: error: req_id={:?}", resp);
            result = Err(ContainerError::General);
        }

        Self::focus_top_container(&state).await.ok();
        let mut next_props = None;
        let name = state.container_state.get_prev_stack().clone().unwrap();
        if let Some(n) = state.container_state.get_container_by_name(&name) {
            next_props = Some(n.clone());
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
        let result;

        if !state
            .container_state
            .contains_stack_by_name(&name.to_string())
        {
            println!("set_visible: Not found in stack: name={}", name);
            return Err(ContainerError::NotFound);
        }

        let item = state.container_state.get_container_by_name(&name.into());
        if let None = item {
            println!("set_visible: Container not found:  name={}", name);
            return Err(ContainerError::NotFound);
        }

        let props = item.unwrap();
        let view_id = props.view_id.clone();
        let resp = ViewManager::set_visibility(&state, view_id, visible).await;
        match resp {
            Ok(_) => {
                result = Ok(ResultType::None);
            }
            Err(_) => {
                error!("set_visible: error: req_id={:?}", resp);
                result = Err(ContainerError::General);
            }
        }

        result
    }

    pub fn on_state_changed(state: &LauncherState, state_change: StateChangeInternal) {
        debug!("on_app_state_change: state_change={:?}", state_change);
        let app_id = state_change.container_props.name.clone();
        if (state_change.states.previous != LifecycleState::Initializing
            && state_change.states.state == LifecycleState::Inactive)
            || state_change.states.state == LifecycleState::Unloading
        {
            let _ = state.container_state.remove_container(app_id);
        }
    }
}
