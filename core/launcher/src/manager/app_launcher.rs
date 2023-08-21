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
    time::{SystemTime, UNIX_EPOCH},
};

use ripple_sdk::{
    api::{
        apps::{
            AppBasicInfo, AppError, AppLaunchInfo, AppManagerResponse, AppResponse, AppRuntime,
            AppRuntimeTransport, AppSession, CloseReason, Dimensions, StateChange,
        },
        device::{
            device_browser::{BrowserNameRequestParams, BrowserRequest},
            device_info_request::DeviceInfoRequest,
            entertainment_data::{HomeIntent, NavigationIntent, NavigationIntentStrict},
        },
        firebolt::{
            fb_discovery::{DiscoveryContext, LaunchRequest},
            fb_lifecycle::LifecycleState,
            fb_lifecycle_management::{
                AppSessionRequest, LifecycleManagementProviderEvent, LifecycleManagementRequest,
                SetStateRequest,
            },
        },
        manifest::{app_library::AppLibrary, apps::AppManifest, device_manifest::RetentionPolicy},
    },
    extn::extn_client_message::ExtnResponse,
    framework::RippleResponse,
    log::{debug, error, info, warn},
    tokio::{
        self,
        time::{sleep, Duration},
    },
    uuid::Uuid,
};
use serde_json::Value;

use crate::{
    launcher_state::LauncherState,
    manager::container_manager::{ContainerManager, StateChangeInternal},
};

use super::{
    container_manager::ContainerProperties,
    container_message::ContainerEvent,
    view_manager::{LaunchParams, ViewManager},
};

use ripple_sdk::futures::future::{BoxFuture, FutureExt};

pub const NAVIGATION_INTENT_PROVIDER_REQUEST: &str = "providerRequest";
#[derive(Debug, Clone)]
struct App {
    #[allow(dead_code)]
    pub manifest: AppManifest,
    pub state: LifecycleState,
    pub app_id: String,
    pub initial_intent: NavigationIntent,
    pub current_intent: NavigationIntent,
    pub launch_params: LaunchParams,
    pub ready: bool,
    pub container_props: ContainerProperties,
    pub session_id: String,
    #[allow(dead_code)]
    pub always_retained: bool,
    pub launch_time: u128,
    pub on_destroyed_action: Option<OnDestroyedAction>,
}

#[derive(Debug, Clone)]
pub enum OnDestroyedAction {
    Launch(LaunchRequest),
}

#[derive(Debug, Clone, Default)]
pub struct AppLauncherState {
    apps: Arc<RwLock<HashMap<String, App>>>,
}

pub struct AppLauncher;

const OFFLINE_APP_CAP: &str = "xrn:firebolt:capability:app:offline";
fn get_app_type(manifest: &AppManifest) -> Option<String> {
    match manifest.runtime.as_str() {
        "web" => Some("HtmlApp".into()),
        "lightning" => match manifest.requires_capability(OFFLINE_APP_CAP) {
            true => Some("ResidentApp".into()),
            false => Some("LightningApp".into()),
        },
        _ => None,
    }
}

impl AppLauncherState {
    fn get_active_instances(self, manifest: &AppManifest) -> usize {
        match get_app_type(manifest) {
            Some(t) => self
                .apps
                .read()
                .unwrap()
                .iter()
                .filter(|(_app_id, app)| app.launch_params._type.eq(&t))
                .count(),
            None => {
                println!("get_active_instances: Invalid manifest");
                0
            }
        }
    }

    fn get_app_len(&self) -> usize {
        self.apps.read().unwrap().len()
    }

    fn get_app_by_id(&self, app_id: &str) -> Option<App> {
        let v = self.apps.read().unwrap();
        let r = v.get(app_id);
        if let Some(r) = r {
            let v = r.clone();
            return Some(v);
        }
        None
    }

    fn get_apps(&self) -> Vec<App> {
        let r = self.apps.read().unwrap();
        r.iter().map(|(_s, app)| app.clone()).collect()
    }

    fn set_app_state(&self, container_id: &str, lifecycle_state: LifecycleState) {
        let mut v = self.apps.write().unwrap();
        let r = v.get_mut(container_id);
        if let Some(r) = r {
            r.state = lifecycle_state
        }
    }

    fn set_app_ready(&self, app_id: &str) {
        let mut v = self.apps.write().unwrap();
        let r = v.get_mut(app_id);
        if let Some(r) = r {
            r.ready = true;
        }
    }

    fn set_app_viewid(&self, container_id: &str, view_id: Uuid) {
        let mut v = self.apps.write().unwrap();
        let r = v.get_mut(container_id);
        if let Some(r) = r {
            r.container_props.view_id = view_id
        }
    }

    fn add_app(&self, key: String, app: App) {
        let mut r = self.apps.write().unwrap();
        r.insert(key, app);
    }

    fn remove_app(&self, key: &str) -> Option<App> {
        let mut r = self.apps.write().unwrap();
        r.remove(key)
    }

    fn always_retained_apps(&self, policy: RetentionPolicy) -> Vec<App> {
        let mut candidates = Vec::new();
        for (_id, app) in self.apps.read().unwrap().iter() {
            if policy.always_retained.contains(&app.app_id) {
                continue;
            }
            candidates.push(app.clone());
        }
        candidates
    }
}

impl AppLauncher {
    pub fn set_state(
        state: LauncherState,
        container_id: String,
        lc_state: LifecycleState,
    ) -> BoxFuture<'static, AppResponse> {
        async move {
            debug!(
                "set_state: container_id={}, state={:?}",
                container_id, state
            );
            let mut final_resp = Ok(AppManagerResponse::None);
            let mut item = state
                .clone()
                .app_launcher_state
                .get_app_by_id(&container_id);

            if let Some(app) = item {
                let previous_state = app.state;
                if previous_state == lc_state {
                    warn!(
                        "set_state app_id={} state={:?} Already in state",
                        app.app_id, lc_state
                    );
                    final_resp = Err(AppError::UnexpectedState);
                } else {
                    state
                        .clone()
                        .app_launcher_state
                        .set_app_state(&container_id, lc_state);

                    if lc_state == LifecycleState::Inactive || lc_state == LifecycleState::Unloading
                    {
                        // add logic to clear inactive and unloading states in main
                        debug!("app is inactive or unloading");
                    }

                    let mut result = serde_json::Map::new();
                    result.insert("state".into(), Value::String(lc_state.as_string().into()));
                    result.insert(
                        "previous".into(),
                        Value::String(previous_state.as_string().into()),
                    );
                    let app_id = app.app_id.clone();

                    // Call Set State Request for Extn Sender
                    if let Err(e) = state
                        .send_extn_request(LifecycleManagementRequest::SetState(SetStateRequest {
                            app_id,
                            state: lc_state,
                        }))
                        .await
                    {
                        error!("Error while setting state {:?}", e);
                    }

                    let state_change = StateChangeInternal {
                        states: StateChange {
                            state: lc_state,
                            previous: previous_state,
                        },
                        container_props: app.container_props.clone(),
                    };
                    Self::on_state_changed(&state, state_change).await;
                }
            } else {
                // Container ID for app not found, check registered providers to
                // see if it's a provider container ID.
                let app_library_state = state.clone().config.app_library_state;
                let resp = AppLibrary::get_provider(&app_library_state, container_id.to_string());
                if resp.is_none() {
                    warn!("set_state: Not found {:?}", container_id);
                    final_resp = Err(AppError::NotFound);
                }
                let provider = resp.unwrap();
                item = state.clone().app_launcher_state.get_app_by_id(&provider);
                if item.is_none() {
                    warn!("set_state: Not found {:?}", container_id);
                    final_resp = Err(AppError::NotFound);
                }
            }

            final_resp
        }
        .boxed()
    }

    pub async fn on_state_changed(state: &LauncherState, state_change: StateChangeInternal) {
        debug!("on_app_state_change: state_change={:?}", state_change);
        let entry = state
            .app_launcher_state
            .get_app_by_id(&state_change.container_props.name);
        if entry.is_none() {
            error!(
                "on_app_state_change: app_id={} Not found",
                state_change.container_props.name
            );
            return;
        }
        let app = entry.unwrap();
        let app_id = app.container_props.name.clone();

        ContainerManager::on_state_changed(state, state_change.clone()).await;

        if state_change.states.previous == LifecycleState::Initializing
            && state_change.states.state == LifecycleState::Inactive
        {
            // Only add the container if app is not launching to suspend and not launched due to
            // a pending provider request.
            if !app.launch_params.suspend {
                if let NavigationIntent::NavigationIntentStrict(
                    NavigationIntentStrict::ProviderRequest(_),
                ) = app.current_intent
                {
                    // Do nothing if the current intent is a provider request
                } else {
                    let props = app.container_props.clone();
                    ContainerManager::add(state, props).await.ok();
                }
            }
            Self::check_retention_policy(state).await;
        } else if state_change.states.state == LifecycleState::Inactive {
            Self::on_inactive(state, &app_id).await.ok();
        } else if state_change.states.state == LifecycleState::Unloading {
            Self::on_unloading(state, &app_id).await.ok();
        }
    }

    async fn check_retention_policy(state: &LauncherState) {
        let policy = state.clone().config.retention_policy;
        let mut app_count_exceeded = false;
        let app_count = state.app_launcher_state.get_app_len() as u64;
        if app_count > policy.max_retained {
            debug!(
                "check_retention_policy: Current app count: {}, Max app count: {}",
                app_count, policy.max_retained
            );
            app_count_exceeded = true;
        }

        if app_count_exceeded {
            Self::remove_oldest_app(state).await;
        }

        loop {
            match Self::get_available_mem_kb(state).await {
                Ok(avail_mem_kb) => {
                    if avail_mem_kb < policy.min_available_mem_kb {
                        if !Self::remove_oldest_app(state).await {
                            warn!("check_retention_policy: No more apps to remove");
                            break;
                        }
                    } else {
                        break;
                    }
                }
                Err(e) => {
                    error!(
                        "check_retention_policy: Could not determine available memory {:?}",
                        e
                    );
                    break;
                }
            }
        }
    }

    async fn get_available_mem_kb(state: &LauncherState) -> Result<u64, AppError> {
        if let Ok(msg) = state
            .send_extn_request(DeviceInfoRequest::AvailableMemory)
            .await
        {
            if let Some(ExtnResponse::Value(v)) = msg.payload.extract() {
                if let Some(memory) = v.as_u64() {
                    return Ok(memory);
                }
            }
        }

        Err(AppError::OsError)
    }

    fn get_oldest_removeable_app(state: &LauncherState) -> Option<String> {
        let policy = state.clone().config.retention_policy;
        let mut candidates = state.app_launcher_state.always_retained_apps(policy);

        let count = candidates.len();

        if count > 1 {
            // Remove oldest entry but never the only one.
            candidates.sort_by(|a, b| a.launch_time.cmp(&b.launch_time));
            return Some(candidates[0].launch_params.name.clone());
        }
        warn!(
            "get_oldest_removeable_app={} Not enough removeable apps",
            count
        );
        None
    }

    async fn remove_oldest_app(state: &LauncherState) -> bool {
        match Self::get_oldest_removeable_app(state) {
            Some(name) => {
                Self::close(state, &name, CloseReason::ResourceContention)
                    .await
                    .ok();
                ContainerManager::remove(state, &name).await.ok();
                true
            }
            None => false,
        }
    }

    async fn on_inactive(
        _state: &LauncherState,
        app_id: &str,
    ) -> Result<AppManagerResponse, AppError> {
        // TODO: This function will be responsibile for determining exactly what to do with application resources
        // based on config data provided by the app manifest (suspend, keep inactive, unload, etc.).

        debug!("on_inactive: entry: app_id={}", app_id);
        Ok(AppManagerResponse::None)
    }

    async fn check_finished(
        state: &LauncherState,
        app_id: &str,
    ) -> Result<AppManagerResponse, AppError> {
        debug!("check_finished: app_id={}", app_id);
        let entry = state.app_launcher_state.get_app_by_id(app_id);
        match entry {
            Some(_app) => {
                warn!(
                    "check_finished={} App not finished unloading, forcing",
                    app_id
                );
                Self::destroy(state, app_id).await
            }
            None => Ok(AppManagerResponse::None),
        }
    }

    async fn on_unloading(
        state: &LauncherState,
        app_id: &str,
    ) -> Result<AppManagerResponse, AppError> {
        debug!("on_unloading: entry: app_id={}", app_id);

        let id = app_id.to_string();
        let timeout = state
            .clone()
            .config
            .lifecycle_policy
            .app_finished_timeout_ms;
        let state_c = state.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(timeout)).await;
            if let Err(e) = Self::check_finished(&state_c, &id).await {
                error!("Error checking finished {:?}", e);
            }
        });

        Ok(AppManagerResponse::None)
    }

    pub async fn on_container_event(state: &LauncherState, event: ContainerEvent) {
        match event {
            ContainerEvent::Focused(previous, next) => {
                if let Some(p) = &previous {
                    if let Some(n) = &next {
                        if p.view_id.eq(&n.view_id) {
                            return;
                        }
                    }
                }

                if let Some(n) = &next {
                    Self::set_state(state.clone(), n.name.clone(), LifecycleState::Foreground)
                        .await
                        .ok();
                }

                if let Some(p) = &previous {
                    if let Some(previous_app) = state.app_launcher_state.get_app_by_id(&p.name) {
                        if previous_app.state == LifecycleState::Foreground {
                            Self::set_state(
                                state.clone(),
                                p.name.clone(),
                                LifecycleState::Background,
                            )
                            .await
                            .ok();
                        }
                    }
                }
            }
            ContainerEvent::Removed(props) => {
                // If the removed container was associated with a launch due to provider request, set the associated app
                // to inactive.
                let iter_apps = state.app_launcher_state.get_apps();
                for app in iter_apps {
                    if app.container_props.view_id == props.view_id {
                        if let NavigationIntent::NavigationIntentStrict(
                            NavigationIntentStrict::ProviderRequest(_),
                        ) = app.current_intent
                        {
                            Self::set_state(state.clone(), props.name, LifecycleState::Inactive)
                                .await
                                .ok();
                        } else {
                            Self::set_state(state.clone(), props.name, LifecycleState::Background)
                                .await
                                .ok();
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    fn get_transport(url: String) -> AppRuntimeTransport {
        if !url.as_str().contains("__firebolt_endpoint") {
            AppRuntimeTransport::Bridge
        } else {
            AppRuntimeTransport::Websocket
        }
    }

    pub async fn pre_launch(
        state: &LauncherState,
        manifest: AppManifest,
        callsign: String,
        intent: NavigationIntent,
    ) -> RippleResponse {
        let session = AppSession {
            app: AppBasicInfo {
                id: manifest.name.clone(),
                title: Some(manifest.name.clone()),
                catalog: manifest.content_catalog.clone(),
                url: Some(manifest.start_page.clone()),
            },
            runtime: Some(AppRuntime {
                id: Some(callsign),
                transport: Self::get_transport(manifest.start_page),
            }),
            launch: AppLaunchInfo {
                intent: Some(intent),
                second_screen: None,
                inactive: false,
            },
        };

        if let Err(e) = state
            .send_extn_request(LifecycleManagementRequest::Session(AppSessionRequest {
                session,
            }))
            .await
        {
            error!("Error while prelaunching {:?}", e);
            return Err(e);
        }
        Ok(())
    }

    pub async fn launch(
        state: &LauncherState,
        request: LaunchRequest,
    ) -> Result<AppManagerResponse, AppError> {
        let resp = AppLibrary::get_manifest(&state.config.app_library_state, &request.app_id);
        if resp.is_none() {
            return Err(AppError::NotFound);
        }

        let app_manifest = resp.unwrap();

        let app_type = get_app_type(&app_manifest);
        if app_type.is_none() {
            return Err(AppError::NotSupported);
        }
        let instances = state
            .clone()
            .app_launcher_state
            .get_active_instances(&app_manifest);
        let bnrp = BrowserNameRequestParams {
            name: app_manifest.name.clone(),
            runtime: app_manifest.runtime.clone(),
            instances,
        };

        let response = state
            .send_extn_request(BrowserRequest::GetBrowserName(bnrp))
            .await;
        if response.is_err() {
            return Err(AppError::NotSupported);
        }

        let callsign =
            if let Some(ExtnResponse::String(callsign)) = response.unwrap().payload.extract() {
                callsign
            } else {
                return Err(AppError::NotSupported);
            };

        let intent = request.intent.unwrap_or_else(|| {
            NavigationIntent::NavigationIntentStrict(NavigationIntentStrict::Home(HomeIntent {
                context: DiscoveryContext {
                    source: "device".into(),
                },
            }))
        });

        if Self::pre_launch(
            state,
            app_manifest.clone(),
            callsign.clone(),
            intent.clone(),
        )
        .await
        .is_err()
        {
            return Err(AppError::IoError);
        }

        let launch_params = LaunchParams {
            uri: app_manifest.start_page.to_string(),
            browser_name: callsign,
            _type: app_type.unwrap(),
            name: app_manifest.name.to_string(),
            suspend: false,
            requires_focus: true,
            x: app_manifest.x,
            y: app_manifest.y,
            w: app_manifest.w,
            h: app_manifest.h,
            properties: app_manifest.properties.clone(),
        };

        let dims = Dimensions {
            x: launch_params.x,
            y: launch_params.y,
            w: launch_params.w,
            h: launch_params.h,
        };

        let policy = state.clone().config.retention_policy;
        let always_retained = policy
            .always_retained
            .iter()
            .find(|app_id| request.app_id.eq(*app_id));

        let mut app = App {
            manifest: app_manifest.clone(),
            state: LifecycleState::Initializing,
            app_id: request.app_id.clone(),
            initial_intent: intent.clone(),
            current_intent: intent,
            launch_params: launch_params.clone(),
            ready: false,
            container_props: ContainerProperties {
                name: launch_params.name.clone(),
                view_id: Uuid::nil(),
                requires_focus: launch_params.requires_focus,
                dimensions: dims,
            },
            session_id: "none".into(),
            always_retained: always_retained.is_some(),
            launch_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            on_destroyed_action: None,
        };

        let existing_app = state.app_launcher_state.get_app_by_id(&request.app_id);
        if let Some(existing) = existing_app {
            app.initial_intent = existing.initial_intent.clone();
            app.state = existing.state;
            app.container_props.view_id = existing.container_props.view_id;
            app.session_id = existing.session_id.clone();
            let container_props = app.container_props.clone();
            // Update manifest map.
            state
                .app_launcher_state
                .add_app(request.app_id.clone(), app);
            match ContainerManager::add(state, container_props).await {
                Ok(_) => return Ok(AppManagerResponse::None),
                Err(_) => return Err(AppError::IoError),
            }
        } else {
            let timeout = state.clone().config.lifecycle_policy.app_ready_timeout_ms;
            let state_c = state.clone();
            let app_id = request.app_id.clone();
            let launch_time = app.launch_time;
            tokio::spawn(async move {
                sleep(Duration::from_millis(timeout)).await;
                let _ = Self::check_ready(&state_c, &app_id, launch_time).await;
            });
        }

        state
            .app_launcher_state
            .add_app(request.app_id.clone(), app);
        // TODO move logic for permission store to Delegated app launcher

        match ViewManager::acquire_view(state, launch_params.clone()).await {
            Ok(view_id) => {
                state
                    .app_launcher_state
                    .set_app_viewid(&request.app_id, view_id);
                Ok(AppManagerResponse::None)
            }
            Err(e) => {
                error!("view acquire failed {:?}", e);
                state.app_launcher_state.remove_app(&request.app_id);
                Err(AppError::General)
            }
        }
    }

    pub async fn ready(
        state: &LauncherState,
        app_id: &str,
    ) -> Result<AppManagerResponse, AppError> {
        let entry = state.app_launcher_state.get_app_by_id(app_id);
        match entry {
            Some(app) => match app.state {
                LifecycleState::Initializing => {
                    state.app_launcher_state.set_app_ready(app_id);
                    Self::set_state(state.clone(), app_id.into(), LifecycleState::Inactive).await
                }
                _ => Err(AppError::UnexpectedState),
            },
            None => Err(AppError::UnexpectedState),
        }
    }

    pub async fn check_ready(
        state: &LauncherState,
        app_id: &str,
        launch_time: u128,
    ) -> Result<AppManagerResponse, AppError> {
        let entry = state.app_launcher_state.get_app_by_id(app_id);
        match entry {
            Some(app) => {
                if launch_time == app.launch_time && !app.ready {
                    warn!("check_ready: App={} not ready, unloading", app_id);
                    Self::close(state, app_id, CloseReason::AppNotReady)
                        .await
                        .ok();
                    return Err(AppError::AppNotReady);
                }
                Ok(AppManagerResponse::None)
            }
            None => {
                error!("check_ready: App={} not found", app_id);
                Err(AppError::NotFound)
            }
        }
    }

    pub async fn close(
        state: &LauncherState,
        app_id: &str,
        reason: CloseReason,
    ) -> Result<AppManagerResponse, AppError> {
        info!("close {:?}", reason);
        match reason {
            CloseReason::UserExit | CloseReason::RemoteButton => {
                Self::set_state(state.clone(), app_id.into(), LifecycleState::Inactive).await
            }
            CloseReason::Error => {
                Self::set_state(state.clone(), app_id.into(), LifecycleState::Unloading).await
            }
            CloseReason::AppNotReady => {
                Self::set_state(state.clone(), app_id.into(), LifecycleState::Unloading).await
            }
            CloseReason::ResourceContention => {
                Self::set_state(state.clone(), app_id.into(), LifecycleState::Unloading).await
            }
        }
    }

    pub async fn destroy(
        state: &LauncherState,
        app_id: &str,
    ) -> Result<AppManagerResponse, AppError> {
        debug!("destroy: entry: app_id={}", app_id);

        if state.app_launcher_state.get_app_by_id(app_id).is_none() {
            error!("destroy app_id={} Not found", app_id);
            return Err(AppError::NotFound);
        }

        let app = state.app_launcher_state.remove_app(app_id).unwrap();
        let view_id = app.container_props.view_id;

        let resp = ViewManager::release_view(state, view_id).await;
        if let Some(action) = app.on_destroyed_action {
            match action {
                OnDestroyedAction::Launch(request) => {
                    Self::launch(state, request).await.ok();
                }
            }
        }
        match resp {
            Ok(_) => Ok(AppManagerResponse::None),
            Err(_) => Err(AppError::General),
        }
    }

    pub async fn provide(
        state: &LauncherState,
        request: LifecycleManagementProviderEvent,
    ) -> AppResponse {
        match request {
            LifecycleManagementProviderEvent::Add(id) => {
                let existing_app = state.app_launcher_state.get_app_by_id(&id);
                if let Some(existing) = existing_app {
                    if ContainerManager::bring_to_front(
                        state,
                        existing.container_props.name.as_str(),
                    )
                    .await
                    .is_ok()
                    {
                        return Ok(AppManagerResponse::None);
                    }
                }
            }
            LifecycleManagementProviderEvent::Remove(id) => {
                let existing_app = state.app_launcher_state.get_app_by_id(&id);
                if let Some(existing) = existing_app {
                    if ContainerManager::send_to_back(state, existing.container_props.name.as_str())
                        .await
                        .is_ok()
                    {
                        return Ok(AppManagerResponse::None);
                    }
                }
            }
        }

        Err(AppError::General)
    }
}
