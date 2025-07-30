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
    env, fs,
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::{
        apps::{AppError, AppManagerResponse, AppMethod, AppSession, AppSession2_0, StateChange},
        device::{device_user_grants_data::EvaluateAt, entertainment_data::NavigationIntent},
        firebolt::{
            fb_capabilities::{DenyReason, DenyReasonWithCap, FireboltPermission},
            fb_discovery::DISCOVERY_EVENT_ON_NAVIGATE_TO,
            fb_lifecycle::{
                Lifecycle2_0AppEvent, Lifecycle2_0AppEventData, LifecycleManagerState,
                LifecycleState, LifecycleStateChangeEvent,
            },
            fb_lifecycle_management::{
                CompletedSessionResponse, PendingSessionResponse, SessionResponse,
                LCM_EVENT_ON_SESSION_TRANSITION_CANCELED,
                LCM_EVENT_ON_SESSION_TRANSITION_COMPLETED,
            },
            fb_metrics::{AppLifecycleState, AppLifecycleStateChange},
            fb_secondscreen::SECOND_SCREEN_EVENT_ON_LAUNCH_REQUEST,
        },
        gateway::rpc_gateway_api::{AppIdentification, CallerSession},
    },
    log::{debug, error, warn},
    serde_json::{self},
    sync_read_lock, sync_write_lock,
    tokio::sync::{mpsc, oneshot},
    utils::{error::RippleError, time_utils::Timer},
    uuid::Uuid,
};
use ripple_sdk::{
    api::{
        apps::{AppRequest, AppResponse},
        device::device_user_grants_data::GrantLifespan,
        firebolt::{
            fb_discovery::LaunchRequest,
            fb_lifecycle_management::{
                LifecycleManagementCloseEvent, LifecycleManagementCloseParameters,
                LifecycleManagementEventRequest, LifecycleManagementFinishedEvent,
                LifecycleManagementFinishedParameters, LifecycleManagementLaunchEvent,
                LifecycleManagementLaunchParameters, LifecycleManagementReadyEvent,
                LifecycleManagementReadyParameters, LCM_EVENT_ON_REQUEST_CLOSE,
                LCM_EVENT_ON_REQUEST_FINISHED, LCM_EVENT_ON_REQUEST_LAUNCH,
                LCM_EVENT_ON_REQUEST_READY,
            },
        },
    },
    log::info,
    tokio::{self, sync::mpsc::Receiver},
};
use serde_json::{json, Value};

use crate::{
    broker::{broker_utils::BrokerUtils, endpoint_broker::BrokerCallback},
    service::{
        apps::app_events::AppEvents,
        extn::ripple_client::RippleClient,
        telemetry_builder::TelemetryBuilder,
        user_grants::{GrantHandler, GrantPolicyEnforcer, GrantState},
    },
    state::{
        bootstrap_state::ChannelsState, cap::permitted_state::PermissionHandler,
        platform_state::PlatformState, session_state::PendingSessionInfo,
    },
    utils::rpc_utils::rpc_await_oneshot,
};

const APP_ID_TITLE_FILE_NAME: &str = "appInfo.json";
const MIGRATED_APPS_FILE_NAME: &str = "migrations.json";
const APP_ID_TITLE_DIR_NAME: &str = "app_info";
const MIGRATED_APPS_DIR_NAME: &str = "apps";

#[derive(Debug, Clone)]
pub struct App {
    pub initial_session: AppSession,
    pub current_session: AppSession,
    pub session_id: String,
    pub state: LifecycleState,
    pub loaded_session_id: String,
    pub active_session_id: Option<String>,
    pub internal_state: Option<AppMethod>,
    pub app_id: String,
    pub app_metrics_version: Option<String>, // Provided by app via call to Metrics.appInfo
    pub is_app_init_params_invoked: bool,
}

#[derive(Debug, Clone)]
pub struct App2_0 {
    // Ripple currently only manages the session ID (app_instance_id) for App Lifecycle 2.0.
    // No additional business logic is implemented.
    // Extend this structure as more AI 2.0-specific features are added.
    pub app_id: String,
    pub current_session: AppSession2_0,
}

#[derive(Debug, Clone, Default)]
pub struct AppManagerState {
    apps: Arc<RwLock<HashMap<String, App>>>,
    // Very useful for internal launcher where the intent might get untagged
    intents: Arc<RwLock<HashMap<String, NavigationIntent>>>,
    // This is a map <app_id, app_title>
    app_title: Arc<RwLock<HashMap<String, String>>>,
    app_title_persist_path: String,
    // This is a map <app_id, app_migrated_state>
    migrated_apps: Arc<RwLock<HashMap<String, Vec<String>>>>,
    migrated_apps_persist_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct AppManagerState2_0 {
    // Ripple currently only manages the session ID (app_instance_id) for App Lifecycle 2.0.
    // No additional business logic is implemented.
    // Extend this structure as more AI 2.0-specific features are added.
    apps: Arc<RwLock<HashMap<String, App2_0>>>,
}

impl AppManagerState2_0 {
    pub fn new() -> Self {
        AppManagerState2_0 {
            apps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get(&self, app_id: &str) -> Option<App2_0> {
        sync_read_lock!(self.apps).get(app_id).cloned()
    }

    pub fn insert(&self, app_id: String, app: App2_0) {
        let _ = sync_write_lock!(self.apps).insert(app_id, app);
    }

    pub fn remove(&self, app_id: &str) -> Option<App2_0> {
        sync_write_lock!(self.apps).remove(app_id)
    }

    pub fn get_app_id_from_session_id(&self, session_id: &str) -> Option<String> {
        if let Some((_, app)) = sync_read_lock!(self.apps)
            .iter()
            .find(|(_, app)| app.current_session.app_instance_id.eq(session_id))
        {
            Some(app.app_id.clone())
        } else {
            None
        }
    }
}

impl AppManagerState {
    pub fn new(saved_dir: &str) -> Self {
        let app_title_persist_path = Self::get_storage_path(saved_dir, APP_ID_TITLE_DIR_NAME);
        let persisted_app_titles =
            Self::load_persisted_data::<String>(&app_title_persist_path, APP_ID_TITLE_FILE_NAME);

        let migrated_apps_persist_path = Self::get_storage_path(saved_dir, MIGRATED_APPS_DIR_NAME);
        let persisted_migrated_apps = Self::load_persisted_data::<Vec<String>>(
            &migrated_apps_persist_path,
            MIGRATED_APPS_FILE_NAME,
        );

        AppManagerState {
            apps: Arc::new(RwLock::new(HashMap::new())),
            intents: Arc::new(RwLock::new(HashMap::new())),
            app_title: Arc::new(RwLock::new(persisted_app_titles)),
            app_title_persist_path,
            migrated_apps: Arc::new(RwLock::new(persisted_migrated_apps)),
            migrated_apps_persist_path,
        }
    }

    fn restore_data_from_storage(storage_path: &str, file_name: &str) -> Result<Value, String> {
        let file_path = std::path::Path::new(storage_path).join(file_name);
        let file = fs::OpenOptions::new()
            .read(true)
            .open(file_path)
            .map_err(|error| format!("Failed to open storage file: {}", error))?;

        let data: Value = serde_json::from_reader(&file)
            .map_err(|error| format!("Failed to read data from storage file: {}", error))?;
        Ok(data)
    }

    fn load_persisted_data<T>(storage_path: &str, file_name: &str) -> HashMap<String, T>
    where
        T: serde::de::DeserializeOwned,
    {
        // let storage_path = Self::get_storage_path(saved_dir);
        let stored_value_res = Self::restore_data_from_storage(storage_path, file_name);
        if let Ok(stored_value) = stored_value_res {
            let map_res: Result<HashMap<String, T>, _> = serde_json::from_value(stored_value);
            map_res.unwrap_or_default()
        } else {
            HashMap::new()
        }
    }

    fn get_storage_path(saved_dir: &str, dir_name: &str) -> String {
        let mut path = std::path::Path::new(saved_dir).join(dir_name);
        if !path.exists() {
            if let Err(err) = fs::create_dir_all(path.clone()) {
                error!(
                    "Could not create directory {} for persisting data err: {:?}, using /tmp/{}/",
                    path.display().to_string(),
                    err,
                    dir_name
                );

                path = std::path::Path::new(&env::temp_dir().display().to_string()).join(dir_name);

                if let Err(err) = fs::create_dir_all(path.clone()) {
                    error!(
                        "Could not create directory {} for persisting data err: {:?}, data will persist in /tmp/",
                        path.display(),
                        err
                    );
                    path = std::path::Path::new("/tmp").to_path_buf();
                }
            }
        }

        path.display().to_string()
    }

    fn persist_data<T>(
        &self,
        data: &Arc<RwLock<HashMap<String, T>>>,
        persist_path: &str,
        file_name: &str,
    ) -> bool
    where
        T: serde::Serialize + std::fmt::Debug,
    {
        let map = { data.read().unwrap() };
        let path = std::path::Path::new(persist_path).join(file_name);
        if let Ok(file) = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
        {
            return serde_json::to_writer_pretty(&file, &serde_json::to_value(&*map).unwrap())
                .is_ok();
        } else {
            error!("unable to create file: {}:", file_name);
        }
        false
    }

    pub fn get_persisted_app_title_for_app_id(&self, app_id: &str) -> Option<String> {
        sync_read_lock!(self.app_title).get(app_id).cloned()
    }

    pub fn persist_app_title(&self, app_id: &str, title: &str) -> bool {
        {
            let _ = self
                .app_title
                .write()
                .unwrap()
                .insert(app_id.to_owned(), title.to_owned());
        }
        self.persist_data(
            &self.app_title,
            &self.app_title_persist_path,
            APP_ID_TITLE_FILE_NAME,
        )
    }

    pub fn get_persisted_migrated_state_for_app_id(&self, app_id: &str) -> Option<Vec<String>> {
        self.migrated_apps.read().unwrap().get(app_id).cloned()
    }

    pub fn persist_migrated_state(&self, app_id: &str, state: String) -> bool {
        {
            let mut migrated_apps = self.migrated_apps.write().unwrap();
            let entry = migrated_apps.entry(app_id.to_owned()).or_default();
            entry.push(state);
        }
        self.persist_data(
            &self.migrated_apps,
            &self.migrated_apps_persist_path,
            MIGRATED_APPS_FILE_NAME,
        )
    }

    pub fn exists(&self, app_id: &str) -> bool {
        self.apps.read().unwrap().contains_key(app_id)
    }

    pub fn get_app_id_from_session_id(&self, session_id: &str) -> Option<String> {
        {
            debug!("apps and sessions {:?}", self.apps.read().unwrap());
        }
        if let Some((_, app)) = self
            .apps
            .read()
            .unwrap()
            .iter()
            .find(|(_, app)| app.session_id.eq(session_id))
        {
            Some(app.app_id.clone())
        } else {
            None
        }
    }

    fn set_session(&self, app_id: &str, session: AppSession) {
        let mut apps = self.apps.write().unwrap();
        if let Some(app) = apps.get_mut(app_id) {
            app.current_session = session
        }
    }

    fn update_active_session(&self, app_id: &str, session: Option<String>) {
        let mut apps = self.apps.write().unwrap();
        if let Some(app) = apps.get_mut(app_id) {
            debug!(
                "Setting session : {{ appId:{} , session:{:?} }}",
                app_id, session
            );
            app.active_session_id = session
        }
    }

    fn set_state(&self, app_id: &str, state: LifecycleState) {
        let mut apps = self.apps.write().unwrap();
        if let Some(app) = apps.get_mut(app_id) {
            app.state = state;
        }
    }

    fn insert(&self, app_id: String, app: App) {
        let mut apps = self.apps.write().unwrap();
        let _ = apps.insert(app_id, app);
    }

    pub fn get(&self, app_id: &str) -> Option<App> {
        self.apps.read().unwrap().get(app_id).cloned()
    }

    fn remove(&self, app_id: &str) -> Option<App> {
        let mut apps = self.apps.write().unwrap();
        apps.remove(app_id)
    }
    fn set_internal_state(&self, app_id: &str, method: AppMethod) {
        let mut apps = self.apps.write().unwrap();
        if let Some(app) = apps.get_mut(app_id) {
            app.internal_state = Some(method);
        }
    }
    fn get_internal_state(&self, app_id: &str) -> Option<AppMethod> {
        let apps = self.apps.read().unwrap();
        if let Some(app) = apps.get(app_id) {
            app.internal_state.clone()
        } else {
            None
        }
    }
    fn store_intent(&self, app_id: &str, intent: NavigationIntent) {
        let mut intents = self.intents.write().unwrap();
        let _ = intents.insert(app_id.to_owned(), intent);
    }

    fn take_intent(&self, app_id: &str) -> Option<NavigationIntent> {
        let mut intents = self.intents.write().unwrap();
        intents.remove(app_id)
    }

    pub fn set_app_metrics_version(&self, app_id: &str, version: String) -> Result<(), AppError> {
        let mut apps = self.apps.write().unwrap();
        if let Some(app) = apps.get_mut(app_id) {
            app.app_metrics_version = Some(version);
            return Ok(());
        }
        Err(AppError::NotFound)
    }
}

pub struct DelegatedLauncherHandler {
    platform_state: PlatformState,
    app_mgr_req_rx: Receiver<AppRequest>,
    timer_map: HashMap<String, Timer>,
}
/*
Tell lifecycle metrics which methods map to which metrics AppLifecycleStates
*/

fn map_event(event: &AppMethod, inactive: bool) -> Option<AppLifecycleState> {
    if inactive {
        map_event_for_inactive_app(event)
    } else {
        map_event_for_default(event)
    }
}

fn map_event_for_default(event: &AppMethod) -> Option<AppLifecycleState> {
    match event {
        AppMethod::Launch(_) => Some(AppLifecycleState::Launching),
        AppMethod::Ready(_) => Some(AppLifecycleState::Foreground),
        AppMethod::Close(_, _) => Some(AppLifecycleState::NotRunning),
        AppMethod::Finished(_) => Some(AppLifecycleState::NotRunning),
        AppMethod::GetLaunchRequest(_) => Some(AppLifecycleState::Initializing),
        _ => None,
    }
}

fn map_event_for_inactive_app(event: &AppMethod) -> Option<AppLifecycleState> {
    match event {
        AppMethod::Launch(_) => Some(AppLifecycleState::Launching),
        AppMethod::Ready(_) => Some(AppLifecycleState::Suspended),
        AppMethod::SetState(_, lifecycle_state) => Some(lifecycle_state.into()),
        AppMethod::Close(_, _) => Some(AppLifecycleState::NotRunning),
        AppMethod::Finished(_) => Some(AppLifecycleState::NotRunning),
        AppMethod::GetLaunchRequest(_) => Some(AppLifecycleState::Initializing),
        _ => None,
    }
}
impl DelegatedLauncherHandler {
    pub fn new(
        channels_state: ChannelsState,
        platform_state: PlatformState,
    ) -> DelegatedLauncherHandler {
        DelegatedLauncherHandler {
            platform_state,
            app_mgr_req_rx: channels_state
                .get_app_mgr_receiver()
                .expect("App Mgr receiver to be available"),
            timer_map: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    fn create_new_app_session(platform_state: PlatformState, event: LifecycleStateChangeEvent) {
        let LifecycleStateChangeEvent {
            app_id,
            app_instance_id,
            navigation_intent,
            ..
        } = event;

        let mut session = AppSession2_0::new(app_id.clone(), app_instance_id);
        if let Some(intent) = navigation_intent {
            session.set_navigation_intent(intent);
        }
        platform_state.lifecycle2_app_state.insert(
            app_id.clone(),
            App2_0 {
                app_id,
                current_session: session,
            },
        );
    }

    #[allow(dead_code)]
    async fn emit_lifecycle_app_event(
        platform_state: PlatformState,
        app_id: &str,
        event_ctor: fn(Lifecycle2_0AppEventData) -> Lifecycle2_0AppEvent,
        old_state: LifecycleManagerState,
        new_state: LifecycleManagerState,
        source: Option<String>,
    ) {
        let event = event_ctor(Lifecycle2_0AppEventData {
            previous: old_state.into(),
            state: new_state.into(),
            source,
        });
        AppEvents::emit_to_app(
            platform_state,
            app_id.to_string(),
            event.as_event_name(),
            &event.as_event_data_json().unwrap_or_default(),
        )
        .await;
    }

    async fn on_app_lifecycle_state_changed(
        platform_state: PlatformState,
        event: LifecycleStateChangeEvent,
    ) {
        info!("on_app_lifecycle_2_state_changed: {:?}", event);
        let LifecycleStateChangeEvent {
            app_id,
            old_state,
            new_state,
            ..
        } = event.clone();

        use Lifecycle2_0AppEvent::*;
        use LifecycleManagerState::*;

        // Update the navigation intent
        if let Some(intent) = &event.navigation_intent {
            // Get the App2_0 instance, update its session, and re-insert it
            if let Some(mut app) = platform_state.lifecycle2_app_state.get(&app_id) {
                app.current_session.set_navigation_intent(intent.clone());
                platform_state
                    .lifecycle2_app_state
                    .insert(app_id.clone(), app);
            }
        }

        match (old_state.clone(), new_state.clone()) {
            (_, Loading) => {
                Self::create_new_app_session(platform_state, event);
            }
            (_, Initializing) => {
                // Ripple does not maintain the state of the app in the AppManagerState2_0
                debug!(
                    "on_app_lifecycle_state_changed : {} is in Initializing state",
                    event.app_id
                );
            }
            (Initializing, Paused) => {
                Self::emit_lifecycle_app_event(
                    platform_state,
                    &app_id,
                    OnStart,
                    old_state,
                    new_state,
                    None,
                )
                .await;
            }
            (Initializing, Suspended) => {
                Self::emit_lifecycle_app_event(
                    platform_state,
                    &app_id,
                    OnStartSuspend,
                    old_state,
                    new_state,
                    None,
                )
                .await;
            }
            (Paused, Active) => {
                Self::emit_lifecycle_app_event(
                    platform_state,
                    &app_id,
                    OnActivate,
                    old_state,
                    new_state,
                    None,
                )
                .await;
            }
            (Active, Paused) => {
                Self::emit_lifecycle_app_event(
                    platform_state,
                    &app_id,
                    OnPause,
                    old_state,
                    new_state,
                    None,
                )
                .await;
            }
            (Paused, Suspended) => {
                Self::emit_lifecycle_app_event(
                    platform_state,
                    &app_id,
                    OnSuspend,
                    old_state,
                    new_state,
                    None,
                )
                .await;
            }
            (Suspended, Paused) => {
                Self::emit_lifecycle_app_event(
                    platform_state,
                    &app_id,
                    OnResume,
                    old_state,
                    new_state,
                    None,
                )
                .await;
            }
            (Suspended, Hibernated) => {
                Self::emit_lifecycle_app_event(
                    platform_state,
                    &app_id,
                    OnHibernate,
                    old_state,
                    new_state,
                    None,
                )
                .await;
            }
            (Hibernated, Suspended) => {
                Self::emit_lifecycle_app_event(
                    platform_state,
                    &app_id,
                    OnRestore,
                    old_state,
                    new_state,
                    None,
                )
                .await;
            }
            (_, Terminating) => {
                // Clean up the app session and the emit onDestroy app event.
                platform_state.lifecycle2_app_state.remove(&app_id);
                Self::emit_lifecycle_app_event(
                    platform_state,
                    &app_id,
                    OnDestroy,
                    old_state,
                    new_state,
                    None,
                )
                .await;
            }
            _ => {
                debug!(
                "on_app_lifecycle_state_changed : Unhandled state transition in Ripple: {:?} -> {:?}",
                old_state.as_string(), new_state.as_string()
            );
            }
        }
    }

    async fn set_up_lifecycle_manager_listener(&mut self) {
        info!("Setting up lifecycle manager thunder listener");
        let state = self.platform_state.clone();
        let (sender, mut recv) = mpsc::channel(10);
        let broker_callback = BrokerCallback { sender };
        BrokerUtils::process_internal_subscription(
            self.platform_state.clone(),
            "lifecycle2.onAppLifecycleStateChanged",
            Some(json!({"listen": true})),
            None,
            Some(broker_callback),
        )
        .await;
        tokio::spawn(async move {
            while let Some(e) = recv.recv().await {
                debug!("Received lifecycle manager output: {:?}", e);
                if let Some(p) = e.data.params {
                    match serde_json::from_value::<LifecycleStateChangeEvent>(p) {
                        Ok(event) => {
                            Self::on_app_lifecycle_state_changed(state.clone(), event).await;
                        }
                        Err(e) => {
                            error!("Failed to deserialize LifecycleStateChangeEvent: {:?}", e);
                        }
                    }
                }
            }
        });
    }

    pub async fn start(&mut self) {
        if std::env::var("RIPPLE_LIFECYCLE_2_ENABLED")
            .ok()
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false)
        {
            self.set_up_lifecycle_manager_listener().await;
        }

        while let Some(data) = self.app_mgr_req_rx.recv().await {
            // App request
            debug!("DelegatedLauncherHandler: App request: data={:?}", data);
            let method = data.method.clone();
            let (resp, app_id) = match data.method.clone() {
                AppMethod::BrowserSession(session) => (
                    self.start_session(session.clone()).await,
                    Some(session.app.id.clone()),
                ),
                AppMethod::SetState(app_id, state) => {
                    (self.set_state(&app_id, state).await, Some(app_id))
                }
                AppMethod::Launch(launch_request) => {
                    if self.platform_state.has_internal_launcher() {
                        // When using internal launcher extension the NavigationIntent structure will get untagged we will use the original
                        // intent in these cases to avoid loss of data
                        self.platform_state
                            .clone()
                            .app_manager_state
                            .clone()
                            .store_intent(
                                &launch_request.app_id,
                                launch_request.get_intent().clone(),
                            );
                    }
                    (
                        self.send_lifecycle_mgmt_event(LifecycleManagementEventRequest::Launch(
                            LifecycleManagementLaunchEvent {
                                parameters: LifecycleManagementLaunchParameters {
                                    app_id: launch_request.app_id.clone(),
                                    intent: Some(launch_request.get_intent().into()),
                                },
                            },
                        ))
                        .await,
                        Some(launch_request.app_id.clone()),
                    )
                }
                AppMethod::Ready(app_id) => {
                    let resp;
                    if let Err(e) = self.ready_check(&app_id) {
                        resp = Err(e)
                    } else {
                        self.send_app_init_events(app_id.as_str()).await;
                        resp = self
                            .send_lifecycle_mgmt_event(LifecycleManagementEventRequest::Ready(
                                LifecycleManagementReadyEvent {
                                    parameters: LifecycleManagementReadyParameters {
                                        app_id: app_id.clone(),
                                    },
                                },
                            ))
                            .await;
                    }
                    TelemetryBuilder::send_app_load_stop(
                        self.platform_state.clone(),
                        app_id.clone(),
                        resp.is_ok(),
                    )
                    .await;
                    (resp, Some(app_id))
                }
                AppMethod::Close(app_id, reason) => (
                    self.send_lifecycle_mgmt_event(LifecycleManagementEventRequest::Close(
                        LifecycleManagementCloseEvent {
                            parameters: LifecycleManagementCloseParameters {
                                app_id: app_id.clone(),
                                reason,
                            },
                        },
                    ))
                    .await,
                    Some(app_id),
                ),
                AppMethod::CheckFinished(app_id) => {
                    (self.check_finished(&app_id).await, Some(app_id))
                }
                AppMethod::Finished(app_id) => {
                    let resp;
                    if let Err(e) = self.finished_check(&app_id) {
                        resp = Err(e)
                    } else {
                        self.send_lifecycle_mgmt_event(LifecycleManagementEventRequest::Finished(
                            LifecycleManagementFinishedEvent {
                                parameters: LifecycleManagementFinishedParameters {
                                    app_id: app_id.clone(),
                                },
                            },
                        ))
                        .await
                        .ok();
                        resp = self.end_session(&app_id).await;
                    }
                    (resp, Some(app_id))
                }
                AppMethod::GetLaunchRequest(app_id) => {
                    (self.get_launch_request(&app_id).await, Some(app_id))
                }
                AppMethod::GetStartPage(app_id) => {
                    (self.get_start_page(app_id.clone()).await, Some(app_id))
                }
                AppMethod::GetAppContentCatalog(app_id) => (
                    self.get_app_content_catalog(app_id.clone()).await,
                    Some(app_id),
                ),
                AppMethod::GetSecondScreenPayload(app_id) => {
                    (self.get_second_screen_payload(&app_id), Some(app_id))
                }
                AppMethod::State(app_id) => (self.get_state(&app_id), Some(app_id)),
                AppMethod::GetAppName(app_id) => (self.get_app_name(app_id.clone()), Some(app_id)),
                AppMethod::NewActiveSession(session) => {
                    let app_id = session.app.id.clone();
                    Self::new_active_session(self.platform_state.clone(), session, true).await;
                    (Ok(AppManagerResponse::None), Some(app_id))
                }
                AppMethod::NewLoadedSession(session) => {
                    let app_id = session.app.id.clone();
                    Self::new_loaded_session(self.platform_state.clone(), session, true).await;
                    (Ok(AppManagerResponse::None), Some(app_id))
                }
                _ => (Err(AppError::NotSupported), None),
            };

            if let Some(id) = app_id {
                let ps_c = self.platform_state.clone();
                let resp_c = resp.clone();
                tokio::spawn(async move {
                    Self::report_app_state_transition(ps_c, &id, &method, resp_c).await;
                });
            }

            if let Err(e) = resp {
                error!("App error {:?}", e);
            }
            if let Err(e) = data.send_response(resp) {
                error!("App response send error {:?}", e);
            }
        }
        error!("App manager receiver loop ended abruptly");
    }

    async fn report_app_state_transition(
        platform_state: PlatformState,
        app_id: &str,
        method: &AppMethod,
        app_manager_response: Result<AppManagerResponse, AppError>,
    ) {
        if let AppMethod::BrowserSession(_) = method {
            if platform_state
                .endpoint_state_sync(|es| es.has_rule("ripple.reportSessionUpdate"))
                .await
            {
                if let Ok(AppManagerResponse::Session(a)) = app_manager_response {
                    let params = serde_json::to_value(a).unwrap();
                    if BrokerUtils::process_internal_main_request(
                        platform_state.clone(),
                        "ripple.reportSessionUpdate",
                        Some(params),
                    )
                    .await
                    .is_err()
                    {
                        error!("Error reporting lifecycle state")
                    }
                }
            }
        }
        //let rules = platform_state.clone();
        //let rules = rules.endpoint_state.clone();
        if platform_state
            .endpoint_state_sync(|es| es.has_rule("ripple.reportLifecycleStateChange"))
            .await
        {
            // if rules
            //     .read()
            //     .await
            //     .has_rule("ripple.reportLifecycleStateChange")
            // {
            let previous_state = platform_state
                .clone()
                .app_manager_state
                .get_internal_state(app_id);

            /*
            Do not forward internal errors from the launch handler as AppErrors. Only forward third-party application error messages as AppErrors.
            TBD: Collaborate with the SIFT team to obtain the appropriate SIFT code for sharing error messages from the AppLauncher.
            */

            let inactive = platform_state
                .clone()
                .app_manager_state
                .get(app_id)
                .map_or(false, |app| app.initial_session.launch.inactive);

            let to_state = map_event(method, inactive);

            if to_state.is_none() {
                /*
                NOOP, the launcher implementation does not care about this state w/respect to metrics */
                return;
            };
            let from_state = match previous_state {
                Some(state) => map_event(&state, inactive),
                None => None,
            };
            let params = serde_json::to_value(AppLifecycleStateChange {
                app_id: app_id.to_owned(),
                new_state: to_state.unwrap(),
                previous_state: from_state,
            })
            .unwrap();
            if BrokerUtils::process_for_app_main_request(
                platform_state.clone(),
                "ripple.reportLifecycleStateChange",
                Some(params),
                app_id,
            )
            .await
            .is_err()
            {
                error!("Error reporting lifecycle state")
            }
        }

        platform_state
            .clone()
            .app_manager_state
            .clone()
            .set_internal_state(app_id, method.clone());
    }

    fn get_state(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
        let manifest = self.platform_state.app_manager_state.get(app_id);
        match manifest {
            Some(app) => Ok(AppManagerResponse::State(app.state)),
            None => Err(AppError::NotFound),
        }
    }

    async fn start_session(
        &mut self,
        mut session: AppSession,
    ) -> Result<AppManagerResponse, AppError> {
        let app_id = session.app.id.clone();

        if let Some(app_title) = session.app.title.as_ref() {
            self.platform_state
                .app_manager_state
                .persist_app_title(app_id.as_str(), app_title);
        }
        if self.platform_state.has_internal_launcher() {
            // Specifically for internal launcher untagged navigation intent will probably not match the original cold launch usecase
            // if there is a stored intent for this case take it and replace it with session
            if let Some(intent) = self.platform_state.app_manager_state.take_intent(&app_id) {
                debug!("Updating intent from initial call {:?}", intent);
                session.update_intent(intent);
            }
        }

        TelemetryBuilder::send_app_load_start(
            self.platform_state.clone(),
            app_id.clone(),
            None,
            None,
        )
        .await;
        debug!("start_session: entry: app_id={}", app_id);
        match self.platform_state.app_manager_state.get(&app_id) {
            Some(app) if (app.state != LifecycleState::Unloading) => {
                Ok(AppManagerResponse::Session(
                    self.precheck_then_load_or_activate(session, false).await,
                ))
            }
            _ => {
                // New loaded Session, Caller must provide Intent.
                if session.launch.intent.is_none() {
                    return Err(AppError::NoIntentError);
                }
                // app is unloading
                if self.platform_state.app_manager_state.get(&app_id).is_some() {
                    // app exist so we are creating a new session
                    // because the other one is unloading, remove the old session now
                    self.end_session(&app_id).await.ok();
                }
                Ok(AppManagerResponse::Session(
                    self.precheck_then_load_or_activate(session, true).await,
                ))
            }
        }
    }

    pub async fn check_grants_then_load_or_activate(
        platform_state: PlatformState,
        pending_session_info: PendingSessionInfo,
        emit_completed: bool,
    ) -> SessionResponse {
        let session = pending_session_info.session;
        let mut perms_with_grants_opt = if !session.launch.inactive {
            Self::get_permissions_requiring_user_grant_resolution(
                platform_state.clone(),
                session.app.id.clone(),
                // Do not pass None as catalog value from this place, instead pass an empty string when app.catalog is None
                Some(session.app.catalog.clone().unwrap_or_default()),
            )
            .await
        } else {
            None
        };
        if perms_with_grants_opt.is_some()
            && !GrantPolicyEnforcer::can_proceed_with_user_grant_resolution(
                platform_state.clone(),
                &perms_with_grants_opt.clone().unwrap(),
            )
            .await
        {
            // reset perms_req_user_grants_opt
            perms_with_grants_opt = None;
        }
        let app_id = session.app.id.clone();
        match perms_with_grants_opt {
            Some(perms_with_grants) => {
                // Grants required, spawn a thread to handle the response from grants
                //let cloned_ps = platform_state.clone();
                let cloned_app_id = session.app.id.clone();
                let spawn_ps = platform_state.clone();
                tokio::spawn(async move {
                    let resolved_result = GrantState::check_with_roles(
                        spawn_ps.clone(),
                        &CallerSession::default(),
                        &AppIdentification {
                            app_id: cloned_app_id.to_owned(),
                        },
                        &perms_with_grants,
                        true,
                        false, // false here as we have already applied user grant exclusion filter.
                        false,
                    )
                    .await;
                    match resolved_result {
                        // Granted and Denied are valid results for app session lifecyclemgmt.
                        Ok(_)
                        | Err(DenyReasonWithCap {
                            reason: DenyReason::GrantDenied,
                            ..
                        }) => {
                            if pending_session_info.loading {
                                Self::new_loaded_session(spawn_ps.clone(), session, true).await;
                            } else {
                                Self::new_active_session(spawn_ps.clone(), session, true).await;
                            }
                        }
                        _ => {
                            debug!("handle session for deferred grant and other errors");
                            Self::emit_cancelled(spawn_ps.clone(), &cloned_app_id).await;
                        }
                    }
                });
                SessionResponse::Pending(PendingSessionResponse {
                    app_id,
                    transition_pending: true,
                    session_id: pending_session_info.session_id,
                    loaded_session_id: pending_session_info.loaded_session_id,
                })
            }
            None => {
                // No grants required, transition immediately
                if pending_session_info.loading {
                    let sess =
                        Self::new_loaded_session(platform_state.clone(), session, emit_completed)
                            .await;
                    SessionResponse::Completed(sess)
                } else {
                    Self::new_active_session(platform_state.clone(), session, emit_completed).await;
                    SessionResponse::Completed(Self::to_completed_session(
                        /*
                        TODO.. unwrap bad - who owns handling this?
                        */
                        &platform_state
                            .clone()
                            .app_manager_state
                            .clone()
                            .get(&app_id)
                            .unwrap(),
                    ))
                }
            }
        }
    }

    /// Does some prechecks like user grants.
    /// If a user grant is not required then the session is created or transitioned
    /// and SessionResponse will return a CompletedSession
    /// If a user grant is required, then a new thread is spawned to handle the grant response.
    /// When the grant is resolved then an AppMethod is sent back to the delegated launcher
    /// to actually perform creating or transitioning the session and update the state in the delegated launcher.
    async fn precheck_then_load_or_activate(
        &mut self,
        session: AppSession,
        loading: bool,
    ) -> SessionResponse {
        let app_id = session.app.id.clone();
        let mut session_id = None;
        let mut loaded_session_id = None;
        if !loading {
            let app = self.platform_state.app_manager_state.get(&app_id).unwrap();
            // unwrap is safe here, when precheck_then_load_or_activate is called with loading=false
            // then the app should have already existed in self.apps

            if session.launch.inactive {
                // Uncommon, moving an already loaded app to inactive again using session
                self.platform_state
                    .app_manager_state
                    .update_active_session(&app_id, None);
                return SessionResponse::Completed(Self::to_completed_session(&app));
            }
            session_id = Some(app.session_id.clone());
            loaded_session_id = Some(app.loaded_session_id);
        }

        let pending_session_info = PendingSessionInfo {
            session: session.clone(),
            loading,
            session_id,
            loaded_session_id,
        };

        self.platform_state
            .session_state
            .add_pending_session(app_id.clone(), Some(pending_session_info.clone()));

        let result = PermissionHandler::fetch_permission_for_app_session(
            self.platform_state.clone(),
            &app_id,
        )
        .await;
        debug!(
            "precheck_then_load_or_activate: fetch_for_app_session completed for app_id={}, result={:?}",
            app_id, result
        );

        if let Err(RippleError::NotAvailable) = result {
            // Permissions not available. PermissionHandler will attempt to resolve. PendingSessionRequestProcessor will
            // refetch permissions and send lifecyclemanagement.OnSessionTransitionComplete when available.
            return SessionResponse::Pending(PendingSessionResponse {
                app_id,
                transition_pending: true,
                session_id: pending_session_info.session_id,
                loaded_session_id: pending_session_info.loaded_session_id,
            });
        }

        Self::check_grants_then_load_or_activate(
            self.platform_state.clone(),
            pending_session_info,
            false,
        )
        .await
    }

    /// Actually perform the transition of the session from inactive to active.
    /// Generate a new active_session_id.
    /// If this transition happened asynchronously, then emit the completed event
    /// If there is an intent in this new session then emit the onNavigateTo event with the intent
    /// If this is session was triggered by a second screen launch, then emit the second screen firebolt event
    async fn new_active_session(
        platform_state: PlatformState,
        session: AppSession,
        emit_event: bool,
    ) {
        let app_id = session.app.id.clone();
        let app = match platform_state.clone().app_manager_state.get(&app_id) {
            Some(app) => app,
            None => return,
        };

        if app.active_session_id.is_none() {
            platform_state
                .clone()
                .app_manager_state
                .update_active_session(&app_id, Some(Uuid::new_v4().to_string()));
        }
        platform_state
            .clone()
            .app_manager_state
            .clone()
            .set_session(&app_id, session.clone());
        if emit_event {
            Self::emit_completed(platform_state.clone(), &app_id).await;
        }
        if let Some(intent) = session.launch.intent {
            AppEvents::emit_to_app(
                platform_state.clone(),
                app_id.clone(),
                DISCOVERY_EVENT_ON_NAVIGATE_TO,
                &serde_json::to_value(intent).unwrap_or_default(),
            )
            .await;
        }

        if let Some(ss) = session.launch.second_screen {
            AppEvents::emit_to_app(
                platform_state.clone(),
                app_id.clone(),
                SECOND_SCREEN_EVENT_ON_LAUNCH_REQUEST,
                &serde_json::to_value(ss).unwrap_or_default(),
            )
            .await;
        }
    }

    /// Create a new cold session
    /// Generate the session_id and loaded_session_id for this app
    /// If this transition happened asynchronously, then emit the completed event
    async fn new_loaded_session(
        platform_state: PlatformState,
        session: AppSession,
        emit_event: bool,
    ) -> CompletedSessionResponse {
        let app_id = session.app.id.clone();
        let session_id = Uuid::new_v4().to_string();
        let loaded_session_id = Uuid::new_v4().to_string();
        let mut active_session_id = None;
        if !session.launch.inactive {
            active_session_id = Some(Uuid::new_v4().to_string());
        }

        debug!(
            "start_session: fetch_for_app_session completed for app_id={}",
            app_id
        );

        let app = App {
            initial_session: session.clone(),
            current_session: session.clone(),
            session_id: session_id.clone(),
            loaded_session_id: loaded_session_id.clone(),
            active_session_id: active_session_id.clone(),
            state: LifecycleState::Initializing,
            internal_state: None,
            app_id: app_id.clone(),
            app_metrics_version: None,
            is_app_init_params_invoked: false,
        };
        platform_state
            .app_manager_state
            .insert(app_id.clone(), app.clone());
        let sess = Self::to_completed_session(&app);
        if emit_event {
            Self::emit_completed(platform_state, &app_id).await;
        }
        sess
    }

    async fn get_permissions_requiring_user_grant_resolution(
        ps: PlatformState,
        app_id: String,
        catalog: Option<String>,
    ) -> Option<Vec<FireboltPermission>> {
        // Get the list of permissions that the calling app currently has
        debug!(" Get the list of permissions that the calling app currently has {app_id}");
        let app_perms = PermissionHandler::get_cached_app_permissions(ps.clone(), &app_id).await;
        if app_perms.is_empty() {
            return None;
        }
        debug!("list of permission that {} has {:?}", &app_id, app_perms);
        // Get the list of grant policies from device manifest.
        let grant_polices_map_opt = ps.clone().get_device_manifest().capabilities.grant_policies;
        debug!(
            "get the list of grant policies from device manifest file:{:?}",
            grant_polices_map_opt
        );

        let grant_polices_map = grant_polices_map_opt?;
        //Filter out the caps only that has evaluate at
        let mut final_perms: Vec<FireboltPermission> = app_perms
            .into_iter()
            .filter(|perm| {
                let filtered_policy_opt = grant_polices_map.get(&perm.cap.as_str());
                if filtered_policy_opt.is_none() {
                    return false;
                }
                if let Some(filtered_policy) = filtered_policy_opt {
                    let policy = match perm.role {
                        ripple_sdk::api::firebolt::fb_capabilities::CapabilityRole::Use => {
                            filtered_policy.use_.as_ref()
                        }
                        ripple_sdk::api::firebolt::fb_capabilities::CapabilityRole::Manage => {
                            filtered_policy.manage.as_ref()
                        }
                        ripple_sdk::api::firebolt::fb_capabilities::CapabilityRole::Provide => {
                            filtered_policy.provide.as_ref()
                        }
                    };
                    debug!("exact policy {:?}", policy);
                    if policy.is_none() {
                        return false;
                    }
                    policy
                        .unwrap()
                        .evaluate_at
                        .contains(&EvaluateAt::ActiveSession)
                } else {
                    false
                }
            })
            .filter(|perm| {
                ps.cap_state
                    .generic
                    .check_supported(&[perm.clone()])
                    .is_ok()
            })
            .collect();
        final_perms = GrantPolicyEnforcer::apply_grant_exclusion_filters(
            ps.clone(),
            &app_id,
            catalog,
            &final_perms,
        )
        .await;
        debug!(
            "list of permissions that need to be evaluated: {:?}",
            final_perms
        );
        if !final_perms.is_empty() {
            // check if grants are resolved after checking for partner exclusion
            let final_perms =
                GrantHandler::are_all_user_grants_resolved(ps.clone(), &app_id, final_perms);
            if !final_perms.is_empty() {
                return Some(final_perms);
            }
        }
        None
    }

    fn to_completed_session(app: &App) -> CompletedSessionResponse {
        CompletedSessionResponse {
            app_id: app.app_id.clone(),
            session_id: app.session_id.clone(),
            loaded_session_id: app.loaded_session_id.clone(),
            active_session_id: app.active_session_id.clone(),
            transition_pending: false,
        }
    }

    pub async fn emit_completed(platform_state: PlatformState, app_id: &String) {
        platform_state.session_state.clear_pending_session(app_id);
        let app = match platform_state.app_manager_state.get(app_id) {
            Some(app) => app,
            None => return,
        };
        let sr = SessionResponse::Completed(Self::to_completed_session(&app));
        AppEvents::emit(
            platform_state,
            LCM_EVENT_ON_SESSION_TRANSITION_COMPLETED,
            &serde_json::to_value(sr).unwrap(),
        )
        .await;
    }

    pub async fn emit_cancelled(platform_state: PlatformState, app_id: &String) {
        platform_state.session_state.clear_pending_session(app_id);
        AppEvents::emit(
            platform_state,
            LCM_EVENT_ON_SESSION_TRANSITION_CANCELED,
            &json!({ "app_id": app_id }),
        )
        .await;
    }

    async fn end_session(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
        debug!("end_session: entry: app_id={}", app_id);
        let app = self.platform_state.app_manager_state.remove(app_id);
        if app.is_some() {
            if let Some(timer) = self.timer_map.remove(app_id) {
                timer.cancel();
            }
        } else {
            error!("end_session app_id={} Not found", app_id);
            return Err(AppError::NotFound);
        }
        Ok(AppManagerResponse::None)
    }

    async fn get_launch_request(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
        match self.platform_state.app_manager_state.get(app_id) {
            Some(mut app) => {
                let launch_request = LaunchRequest {
                    app_id: app.initial_session.app.id.clone(),
                    intent: app.initial_session.launch.intent.clone(),
                };
                app.is_app_init_params_invoked = true;
                self.platform_state
                    .app_manager_state
                    .insert(app_id.to_string(), app);
                Ok(AppManagerResponse::LaunchRequest(launch_request))
            }
            None => {
                error!("get_launch_request failed for app_id={}, Notfound", app_id);
                Err(AppError::NotFound)
            }
        }
    }

    pub async fn send_app_init_events(&self, app_id: &str) {
        if let Some(app) = self.platform_state.app_manager_state.get(app_id) {
            if self
                .platform_state
                .get_device_manifest()
                .get_lifecycle_configuration()
                .is_emit_event_on_app_init_enabled()
                && !app.is_app_init_params_invoked
            {
                if let Some(intent) = app.initial_session.launch.intent.clone() {
                    AppEvents::emit_to_app(
                        self.platform_state.clone(),
                        app_id.to_string(),
                        DISCOVERY_EVENT_ON_NAVIGATE_TO,
                        &serde_json::to_value(intent).unwrap_or_default(),
                    )
                    .await;
                }
                if let Some(ss) = app.initial_session.launch.second_screen.clone() {
                    AppEvents::emit_to_app(
                        self.platform_state.clone(),
                        app_id.to_string(),
                        SECOND_SCREEN_EVENT_ON_LAUNCH_REQUEST,
                        &serde_json::to_value(ss).unwrap_or_default(),
                    )
                    .await;
                }
            }
        }
    }
    async fn set_state(
        &mut self,
        app_id: &str,
        state: LifecycleState,
    ) -> Result<AppManagerResponse, AppError> {
        debug!("set_state: entry: app_id={}, state={:?}", app_id, state);
        let am_state = self.platform_state.clone().app_manager_state.clone();
        let app = match am_state.get(app_id) {
            Some(app) => app,
            None => {
                warn!("appid:{} Not found", app_id);
                return Err(AppError::NotFound);
            }
        };

        let previous_state = app.state;

        if previous_state == state {
            warn!(
                "set_state app_id:{} state:{:?} Already in state",
                app_id, state
            );
            return Err(AppError::UnexpectedState);
        }

        // validate other transition cases
        if self
            .platform_state
            .clone()
            .get_device_manifest()
            .configuration
            .default_values
            .lifecycle_transition_validate
        {
            info!(
                "Calling is_valid_lifecycle_transition for app_id:{} prev state:{:?} state{:?}",
                app_id, previous_state, state
            );
            if !Self::is_valid_lifecycle_transition(previous_state, state) {
                warn!(
                    "set_state app_id:{} prev state:{:?} state{:?} Cannot transition",
                    app_id, previous_state, state
                );
                return Err(AppError::UnexpectedState);
            }
        }

        if state == LifecycleState::Inactive || state == LifecycleState::Unloading {
            self.platform_state.clone().cap_state.grant_state.custom_delete_entries(app_id.into(), |grant_entry| -> bool {
                !(matches!(&grant_entry.lifespan, Some(entry_lifespan) if entry_lifespan == &GrantLifespan::AppActive))
            });
        }
        warn!(
            "set_state app_id:{} prev state:{:?} state{:?}",
            app_id, previous_state, state
        );
        am_state.set_state(app_id, state);
        // remove active session id when the app is going back to inactive (not going to inactive for first time)
        if (previous_state != LifecycleState::Initializing) && (state == LifecycleState::Inactive) {
            am_state.update_active_session(app_id, None);
        }

        let state_change = StateChange {
            state,
            previous: previous_state,
        };
        let event_name = state.as_event();
        AppEvents::emit_to_app(
            self.platform_state.clone(),
            app_id.to_string(),
            event_name,
            &serde_json::to_value(state_change).unwrap(),
        )
        .await;

        if LifecycleState::Unloading == state {
            self.on_unloading(app_id).await.ok();
        }

        // Check if the device manifest is enabled with events to emit discovery.navigateTo
        // if an app is coming back to active from Inactive.
        // This is necessary as some apps do not run processes to update the navigation
        // intent to conserve memory footprint
        if self
            .platform_state
            .clone()
            .get_device_manifest()
            .lifecycle
            .is_emit_navigate_on_activate()
            && previous_state == LifecycleState::Inactive
            && matches!(
                state,
                LifecycleState::Background | LifecycleState::Foreground
            )
        {
            let session = app.current_session.clone();
            if let Some(intent) = session.launch.intent {
                AppEvents::emit_to_app(
                    self.platform_state.clone(),
                    app_id.to_owned(),
                    DISCOVERY_EVENT_ON_NAVIGATE_TO,
                    &serde_json::to_value(intent).unwrap_or_default(),
                )
                .await;
            }
        }
        Ok(AppManagerResponse::None)
    }

    fn ready_check(&self, app_id: &str) -> AppResponse {
        let app = match self
            .platform_state
            .clone()
            .app_manager_state
            .clone()
            .get(app_id)
        {
            Some(app) => app,
            None => {
                warn!("appid:{} Not found", app_id);
                return Err(AppError::NotFound);
            }
        };
        match app.state {
            LifecycleState::Initializing => Ok(AppManagerResponse::None),
            _ => Err(AppError::UnexpectedState),
        }
    }

    fn finished_check(&self, app_id: &str) -> AppResponse {
        let app = match self
            .platform_state
            .clone()
            .app_manager_state
            .clone()
            .get(app_id)
        {
            Some(app) => app,
            None => {
                warn!("appid:{} Not found", app_id);
                return Err(AppError::NotFound);
            }
        };
        match app.state {
            LifecycleState::Unloading => Ok(AppManagerResponse::None),
            _ => Err(AppError::UnexpectedState),
        }
    }

    async fn send_lifecycle_mgmt_event(
        &mut self,
        event: LifecycleManagementEventRequest,
    ) -> Result<AppManagerResponse, AppError> {
        if self.platform_state.clone().has_internal_launcher() {
            if let Err(e) = self.platform_state.clone().get_client().send_event(event) {
                error!("send event error {:?}", e);
                return Err(AppError::OsError);
            }
        } else {
            let event_name;
            let value;
            match event {
                LifecycleManagementEventRequest::Launch(req) => {
                    event_name = LCM_EVENT_ON_REQUEST_LAUNCH;
                    value = serde_json::to_value(req).unwrap();
                }
                LifecycleManagementEventRequest::Ready(req) => {
                    event_name = LCM_EVENT_ON_REQUEST_READY;
                    value = serde_json::to_value(req).unwrap();
                }
                LifecycleManagementEventRequest::Close(req) => {
                    event_name = LCM_EVENT_ON_REQUEST_CLOSE;
                    value = serde_json::to_value(req).unwrap();
                }
                LifecycleManagementEventRequest::Finished(req) => {
                    event_name = LCM_EVENT_ON_REQUEST_FINISHED;
                    value = serde_json::to_value(req).unwrap();
                }
                _ => return Err(AppError::OsError),
            }

            AppEvents::emit(self.platform_state.clone(), event_name, &value).await;
        }

        Ok(AppManagerResponse::None)
    }

    async fn get_start_page(&mut self, app_id: String) -> Result<AppManagerResponse, AppError> {
        match self
            .platform_state
            .clone()
            .app_manager_state
            .clone()
            .get(&app_id)
        {
            Some(app) => Ok(AppManagerResponse::StartPage(app.initial_session.app.url)),
            None => Err(AppError::NotFound),
        }
    }

    async fn start_timer(helper: Arc<RippleClient>, timeout_ms: u64, method: AppMethod) -> Timer {
        let cb = async move {
            let (resp_tx, resp_rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();
            let req = AppRequest::new(method, resp_tx);
            if let Err(e) = helper.clone().send_app_request(req) {
                error!("Failed to send app request after timer expired: {:?}", e);
            }

            if rpc_await_oneshot(resp_rx).await.is_ok() {
                debug!("timer complete")
            }
        };
        Timer::start(timeout_ms, cb)
    }

    async fn on_unloading(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
        debug!("on_unloading: entry: app_id={}", app_id);

        if self.platform_state.app_manager_state.exists(app_id) {
            let client = self.platform_state.get_client();
            let timeout = self
                .platform_state
                .get_device_manifest()
                .get_lifecycle_policy()
                .app_finished_timeout_ms;
            let unloading_timer = Self::start_timer(
                client,
                timeout,
                AppMethod::CheckFinished(app_id.to_string()),
            )
            .await;
            self.timer_map.insert(app_id.to_string(), unloading_timer);
        }

        Ok(AppManagerResponse::None)
    }

    async fn check_finished(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
        debug!("check_finished: app_id={}", app_id);
        let entry = self.platform_state.app_manager_state.get(app_id);
        match entry {
            Some(_app) => {
                warn!(
                    "check_finished app_id:{} App not finished unloading, forcing",
                    app_id
                );
                self.end_session(app_id).await
            }
            None => Ok(AppManagerResponse::None),
        }
    }

    fn get_second_screen_payload(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
        if let Some(app) = self.platform_state.app_manager_state.get(app_id) {
            let mut payload = "".to_string();
            if let Some(sse) = &app.current_session.launch.second_screen {
                if let Some(data) = &sse.data {
                    payload = data.to_string();
                }
            }
            return Ok(AppManagerResponse::SecondScreenPayload(payload));
        }
        Err(AppError::NotFound)
    }

    async fn get_app_content_catalog(
        &mut self,
        app_id: String,
    ) -> Result<AppManagerResponse, AppError> {
        match self.platform_state.app_manager_state.get(&app_id) {
            Some(app) => Ok(AppManagerResponse::AppContentCatalog(
                app.initial_session.app.catalog,
            )),
            None => Err(AppError::NotFound),
        }
    }

    fn get_app_name(&mut self, app_id: String) -> Result<AppManagerResponse, AppError> {
        match self.platform_state.app_manager_state.get(&app_id) {
            Some(app) => Ok(AppManagerResponse::AppName(app.initial_session.app.title)),
            None => {
                match self
                    .platform_state
                    .app_manager_state
                    .get_persisted_app_title_for_app_id(&app_id)
                {
                    Some(app_title) => Ok(AppManagerResponse::AppName(Some(app_title))),
                    None => Err(AppError::NotFound),
                }
            }
        }
    }

    fn is_valid_lifecycle_transition(from: LifecycleState, to: LifecycleState) -> bool {
        // Early exit, Not do the state transition when from and to States are the same
        if from == to {
            return false;
        }

        match (from, to) {
            // Allow transitioning from initializing to only Inactive
            (LifecycleState::Initializing, _) => to == LifecycleState::Inactive,
            // An app MUST NOT be transitioned to Suspended, or Unloaded (i.e. not running anymore)
            // from any state other than Inactive
            (_, LifecycleState::Suspended | LifecycleState::Unloading) => {
                from == LifecycleState::Inactive
            }
            // An app MUST NOT be transitioned (or immediate set to) Foreground
            // without going through either Inactive or Background
            (_, LifecycleState::Foreground) => {
                from == LifecycleState::Inactive || from == LifecycleState::Background
            }
            // Transition from Suspended to only Inactive is allowed
            (LifecycleState::Suspended, LifecycleState::Inactive) => true,
            (LifecycleState::Suspended, _) => false,
            // Do not allow transition to initializing from any other state.
            (_, LifecycleState::Initializing) => false,
            // No more state transitions are allowed from Unloading
            (LifecycleState::Unloading, _) => false,
            // Allow any other state transition
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_state_transition() {
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Inactive,
            LifecycleState::Inactive
        ),);
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Foreground,
            LifecycleState::Foreground
        ),);
    }

    #[test]
    fn test_transition_from_initializing() {
        // Initializing to Inactive
        assert!(DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Initializing,
            LifecycleState::Inactive
        ),);
        // Initializing to Foreground
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Initializing,
            LifecycleState::Foreground
        ),);
        // Initializing to Background
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Initializing,
            LifecycleState::Background
        ),);
        // Initializing to Suspended
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Initializing,
            LifecycleState::Suspended
        ),);
        // Initializing to Unloading
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Initializing,
            LifecycleState::Unloading
        ),);
    }

    #[test]
    fn test_transition_from_inactive() {
        // Inactive to Background
        assert!(DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Inactive,
            LifecycleState::Background
        ),);
        // Inactive to Foreground
        assert!(DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Inactive,
            LifecycleState::Foreground
        ),);
        // Inactive to Suspended
        assert!(DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Inactive,
            LifecycleState::Suspended
        ),);
        // Inactive to Unloading
        assert!(DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Inactive,
            LifecycleState::Unloading
        ),);
        // Inactive to Initializing
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Inactive,
            LifecycleState::Initializing
        ),);
    }

    #[test]
    fn test_transition_from_foreground() {
        // Foreground to Background
        assert!(DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Foreground,
            LifecycleState::Background
        ),);
        // Foreground to Inactive
        assert!(DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Foreground,
            LifecycleState::Inactive
        ),);
        // Foreground to Suspended
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Foreground,
            LifecycleState::Suspended
        ),);
        // Foreground to Unloading
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Foreground,
            LifecycleState::Unloading
        ),);
        // Foreground to Initializing
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Foreground,
            LifecycleState::Initializing
        ),);
    }

    #[test]
    fn test_transition_from_background() {
        // Background to Foreground
        assert!(DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Background,
            LifecycleState::Foreground
        ),);
        // Background to Inactive
        assert!(DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Background,
            LifecycleState::Inactive
        ),);
        // Background to Suspended
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Background,
            LifecycleState::Suspended
        ),);
        // Background to Unloading
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Background,
            LifecycleState::Unloading
        ),);
        // Background to Initializing
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Background,
            LifecycleState::Initializing
        ),);
    }

    #[test]
    fn test_transition_from_suspended() {
        // Suspended to Inactive
        assert!(DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Suspended,
            LifecycleState::Inactive
        ),);
        // Suspended to Foreground
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Suspended,
            LifecycleState::Foreground
        ),);
        // Suspended to Background
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Suspended,
            LifecycleState::Background
        ),);
        // Suspended to Unloading
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Suspended,
            LifecycleState::Unloading
        ),);
        // Suspended to Initializing
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Suspended,
            LifecycleState::Initializing
        ),);
    }
    #[test]
    fn test_transition_from_unloading() {
        // Unloading to Inactive
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Unloading,
            LifecycleState::Inactive
        ),);
        // Unloading to Foreground
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Unloading,
            LifecycleState::Foreground
        ),);
        // Unloading to Background
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Unloading,
            LifecycleState::Background
        ),);
        // Unloading to Suspended
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Unloading,
            LifecycleState::Suspended
        ),);
        // Unloading to Initializing
        assert!(!DelegatedLauncherHandler::is_valid_lifecycle_transition(
            LifecycleState::Unloading,
            LifecycleState::Initializing
        ),);
    }
}
