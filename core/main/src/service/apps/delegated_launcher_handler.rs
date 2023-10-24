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
        apps::{
            AppError, AppManagerResponse, AppMethod, AppSession, EffectiveTransport, StateChange,
        },
        device::{device_user_grants_data::EvaluateAt, entertainment_data::NavigationIntent},
        firebolt::{
            fb_capabilities::FireboltPermission,
            fb_discovery::DISCOVERY_EVENT_ON_NAVIGATE_TO,
            fb_lifecycle::LifecycleState,
            fb_lifecycle_management::{
                CompletedSessionResponse, PendingSessionResponse, SessionResponse,
                LCM_EVENT_ON_SESSION_TRANSITION_CANCELED,
                LCM_EVENT_ON_SESSION_TRANSITION_COMPLETED,
            },
            fb_secondscreen::SECOND_SCREEN_EVENT_ON_LAUNCH_REQUEST,
        },
        gateway::rpc_gateway_api::{AppIdentification, CallerSession},
    },
    log::{debug, error, warn},
    serde_json::{self},
    tokio::sync::oneshot,
    utils::time_utils::Timer,
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
        protocol::{BridgeProtocolRequest, BridgeSessionParams},
    },
    log::info,
    tokio::{self, sync::mpsc::Receiver},
};
use serde_json::json;

use crate::{
    service::{
        apps::app_events::AppEvents, extn::ripple_client::RippleClient,
        telemetry_builder::TelemetryBuilder, user_grants::GrantState,
    },
    state::{
        bootstrap_state::ChannelsState, cap::permitted_state::PermissionHandler,
        platform_state::PlatformState, session_state::Session,
    },
    utils::rpc_utils::rpc_await_oneshot,
};

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
}

#[derive(Debug, Clone, Default)]
pub struct AppManagerState {
    apps: Arc<RwLock<HashMap<String, App>>>,
    // Very useful for internal launcher where the intent might get untagged
    intents: Arc<RwLock<HashMap<String, NavigationIntent>>>
}

impl AppManagerState {
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

    fn store_intent(&self, app_id: &str, intent:NavigationIntent) {
        let mut intents = self.intents.write().unwrap();
        let _ = intents.insert(app_id.to_owned(), intent);
    }

    fn take_intent(&self, app_id: &str) -> Option<NavigationIntent> {
        let mut intents = self.intents.write().unwrap();
        intents.remove(app_id)
    }
}

pub struct DelegatedLauncherHandler {
    platform_state: PlatformState,
    app_mgr_req_rx: Receiver<AppRequest>,
    timer_map: HashMap<String, Timer>,
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

    pub async fn start(&mut self) {
        while let Some(data) = self.app_mgr_req_rx.recv().await {
            // App request
            debug!("DelegatedLauncherHandler: App request: data={:?}", data);
            let resp;
            match data.method.clone() {
                AppMethod::BrowserSession(session) => {
                    resp = self.start_session(session).await;
                }
                AppMethod::SetState(app_id, state) => {
                    resp = self.set_state(&app_id, state).await;
                }
                AppMethod::Launch(launch_request) => {
                    if self.platform_state.has_internal_launcher() {
                        // When using internal launcher extension the NavigationIntent structure will get untagged we will use the original
                        // intent in these cases to avoid loss of data
                        self.platform_state.app_manager_state.store_intent(&launch_request.app_id, launch_request.get_intent().clone());
                    }
                    resp = self
                        .send_lifecycle_mgmt_event(LifecycleManagementEventRequest::Launch(
                            LifecycleManagementLaunchEvent {
                                parameters: LifecycleManagementLaunchParameters { app_id: launch_request.app_id.clone(), intent: Some(launch_request.get_intent().into()) }
                            },
                        ))
                        .await;
                }
                AppMethod::Ready(app_id) => {
                    if let Err(e) = self.ready_check(&app_id) {
                        resp = Err(e)
                    } else {
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
                    TelemetryBuilder::send_app_load_stop(&self.platform_state, app_id, resp.is_ok())
                }
                AppMethod::Close(app_id, reason) => {
                    resp = self
                        .send_lifecycle_mgmt_event(LifecycleManagementEventRequest::Close(
                            LifecycleManagementCloseEvent {
                                parameters: LifecycleManagementCloseParameters { app_id, reason },
                            },
                        ))
                        .await;
                }
                AppMethod::CheckFinished(app_id) => {
                    resp = self.check_finished(&app_id).await;
                }
                AppMethod::Finished(app_id) => {
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
                }
                AppMethod::GetLaunchRequest(app_id) => {
                    resp = self.get_launch_request(&app_id).await;
                }
                AppMethod::GetStartPage(app_id) => {
                    resp = self.get_start_page(app_id).await;
                }
                AppMethod::GetAppContentCatalog(app_id) => {
                    resp = self.get_app_content_catalog(app_id).await;
                }
                AppMethod::GetSecondScreenPayload(app_id) => {
                    resp = self.get_second_screen_payload(&app_id);
                }
                AppMethod::State(app_id) => resp = self.get_state(&app_id),
                AppMethod::GetAppName(app_id) => {
                    resp = self.get_app_name(app_id);
                }
                AppMethod::NewActiveSession(session) => {
                    self.new_active_session(session, true).await;
                    resp = Ok(AppManagerResponse::None);
                }
                AppMethod::NewLoadedSession(session) => {
                    self.new_loaded_session(session, true).await;
                    resp = Ok(AppManagerResponse::None);
                }
                _ => resp = Err(AppError::NotSupported),
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

    fn get_state(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
        let manifest = self.platform_state.app_manager_state.get(app_id);
        match manifest {
            Some(app) => Ok(AppManagerResponse::State(app.state)),
            None => Err(AppError::NotFound),
        }
    }

    async fn start_session(&mut self, mut session: AppSession) -> Result<AppManagerResponse, AppError> {
        let transport = session.get_transport();
        if let EffectiveTransport::Bridge(_) = transport.clone() {
            if !self.platform_state.supports_bridge() {
                error!("Bridge is not a supported contract");
                return Err(AppError::NotSupported);
            }
        }
        // TODO: Add metrics helper for app loading

        let app_id = session.app.id.clone();
        
        if self.platform_state.has_internal_launcher(){
            // Specifically for internal launcher untagged navigation intent will probably not match the original cold launch usecase
            // if there is a stored intent for this case take it and replace it with session
            if let Some(intent) = self.platform_state.app_manager_state.take_intent(&app_id){
                session.update_intent(intent);
            }
        }


        TelemetryBuilder::send_app_load_start(&self.platform_state, app_id.clone(), None, None);
        debug!("start_session: entry: app_id={}", app_id);
        match self.platform_state.app_manager_state.get(&app_id) {
            Some(app) if (app.state != LifecycleState::Unloading) => {
                Ok(AppManagerResponse::Session(
                    self.precheck_then_load_or_activate(session, false).await,
                ))
            }
            _ => {
                if self.platform_state.app_manager_state.get(&app_id).is_some() {
                    // app exist so we are creating a new session
                    // because the other one is unloading, remove the old session now
                    self.end_session(&app_id).await.ok();
                }
                // New loaded Session, Caller must provide Intent.
                // app is unloading
                Ok(AppManagerResponse::Session(
                    self.precheck_then_load_or_activate(session, true).await,
                ))
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
        let perms_with_grants_opt = if !session.launch.inactive {
            Self::check_user_grants_for_active_session(&self.platform_state, session.app.id.clone())
                .await
        } else {
            None
        };
        match perms_with_grants_opt {
            Some(perms_with_grants) => {
                // Grants required, spawn a thread to handle the response from grants
                let cloned_ps = self.platform_state.clone();
                let cloned_app_id = app_id.to_owned();
                tokio::spawn(async move {
                    let resolved_result = GrantState::check_with_roles(
                        &cloned_ps,
                        &CallerSession::default(),
                        &AppIdentification {
                            app_id: cloned_app_id.to_owned(),
                        },
                        &perms_with_grants,
                        false,
                    )
                    .await;
                    if let Err(_deny_reason) = resolved_result {
                        debug!("handle session for deferred grant");
                        AppEvents::emit(
                            &cloned_ps,
                            LCM_EVENT_ON_SESSION_TRANSITION_CANCELED,
                            &json!({ "app_id": cloned_app_id }),
                        )
                        .await;
                    } else {
                        let (resp_tx, resp_rx) =
                            oneshot::channel::<Result<AppManagerResponse, AppError>>();
                        let app_method = if loading {
                            AppMethod::NewLoadedSession(session)
                        } else {
                            AppMethod::NewActiveSession(session)
                        };
                        if cloned_ps
                            .get_client()
                            .send_app_request(AppRequest::new(app_method, resp_tx))
                            .is_ok()
                        {
                            let _ = rpc_await_oneshot(resp_rx).await.is_ok();
                        }
                    }
                });
                SessionResponse::Pending(PendingSessionResponse {
                    app_id,
                    transition_pending: true,
                    session_id,
                    loaded_session_id,
                })
            }
            None => {
                // No grants required, transition immediately
                if loading {
                    let sess = self.new_loaded_session(session, false).await;
                    SessionResponse::Completed(sess)
                } else {
                    self.new_active_session(session, false).await;
                    SessionResponse::Completed(Self::to_completed_session(
                        &self.platform_state.app_manager_state.get(&app_id).unwrap(),
                    ))
                }
            }
        }
    }

    /// Actually perform the transition of the session from inactive to active.
    /// Generate a new active_session_id.
    /// If this transition happened asynchronously, then emit the completed event
    /// If there is an intent in this new session then emit the onNavigateTo event with the intent
    /// If this is session was triggered by a second screen launch, then emit the second screen firebolt event
    async fn new_active_session(&mut self, session: AppSession, emit_event: bool) {
        let app_id = session.app.id.clone();
        let app_opt = self.platform_state.app_manager_state.get(&app_id);
        if app_opt.is_none() {
            return;
        }
        let app = app_opt.unwrap();
        if app.active_session_id.is_none() {
            self.platform_state
                .app_manager_state
                .update_active_session(&app_id, Some(Uuid::new_v4().to_string()));
        }
        self.platform_state
            .app_manager_state
            .set_session(&app_id, session.clone());
        if emit_event {
            self.emit_completed(app_id.clone()).await;
        }

        AppEvents::emit_to_app(
            &self.platform_state,
            app_id.clone(),
            DISCOVERY_EVENT_ON_NAVIGATE_TO,
            &serde_json::to_value(session.launch.intent).unwrap(),
        )
        .await;

        if let Some(ss) = session.launch.second_screen {
            AppEvents::emit_to_app(
                &self.platform_state,
                app_id.clone(),
                SECOND_SCREEN_EVENT_ON_LAUNCH_REQUEST,
                &serde_json::to_value(ss).unwrap(),
            )
            .await;
        }
    }

    /// Create a new cold session
    /// Generate the session_id and loaded_session_id for this app
    /// If this transition happened asynchronously, then emit the completed event
    async fn new_loaded_session(
        &mut self,
        session: AppSession,
        emit_event: bool,
    ) -> CompletedSessionResponse {
        let app_id = session.app.id.clone();
        let transport = session.get_transport();
        let session_id = Uuid::new_v4().to_string();
        if let EffectiveTransport::Bridge(container_id) = transport.clone() {
            if self.platform_state.supports_bridge() {
                // Step 1: Add the session of the app to the state if bridge
                let session_state = Session::new(app_id.clone(), None, transport);
                self.platform_state
                    .session_state
                    .add_session(session_id.clone(), session_state);
                let id = container_id.clone();
                debug!(
                    "App session details: appId: {} session: {}",
                    app_id, session_id
                );
                // Step 2: Start the session using contract
                let request = BridgeSessionParams {
                    container_id,
                    session_id: session_id.clone(),
                    app_id: app_id.clone(),
                };
                let request = BridgeProtocolRequest::StartSession(request);
                let client = self.platform_state.get_client();
                // After processing the session response the launcher will launch the app
                // Below thread is going to wait for the app to be launched and create a connection
                tokio::spawn(async move {
                    if let Err(e) = client.send_extn_request(request).await {
                        error!("Error sending request to bridge {:?}", e);
                    } else {
                        info!("Bridge connected for {}", id);
                    }
                });
            }
        }

        let loaded_session_id = Uuid::new_v4().to_string();
        let mut active_session_id = None;
        if !session.launch.inactive {
            active_session_id = Some(Uuid::new_v4().to_string());
        }

        let platform_state_c = self.platform_state.clone();
        let app_id_c = app_id.clone();
        // Fetch permissions on separate thread
        tokio::spawn(async move {
            if PermissionHandler::fetch_and_store(&platform_state_c, &app_id_c)
                .await
                .is_err()
            {
                error!("Couldnt load permissions for app {}", app_id_c)
            }
        });
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
        };
        self.platform_state
            .app_manager_state
            .insert(app_id.clone(), app.clone());
        let sess = Self::to_completed_session(&app);
        if emit_event {
            self.emit_completed(app_id.clone()).await;
        }
        sess
    }

    async fn check_user_grants_for_active_session(
        ps: &PlatformState,
        app_id: String,
    ) -> Option<Vec<FireboltPermission>> {
        // Get the list of permissions that the calling app currently has
        debug!(" Get the list of permissions that the calling app currently has {app_id}");
        let app_perms = PermissionHandler::get_app_permission(ps, &app_id).await;
        if app_perms.is_empty() {
            return None;
        }
        debug!("list of permission that {} has {:?}", &app_id, app_perms);
        // Get the list of grant policies from device manifest.
        let grant_polices_map_opt = ps.get_device_manifest().capabilities.grant_policies;
        debug!(
            "get the list of grant policies from device manifest file:{:?}",
            grant_polices_map_opt
        );
        grant_polices_map_opt.as_ref()?;
        let grant_polices_map = grant_polices_map_opt.unwrap();
        //Filter out the caps only that has evaluate at
        let final_perms: Vec<FireboltPermission> = app_perms
            .into_iter()
            .filter(|perm| {
                let filtered_policy_opt = grant_polices_map.get(&perm.cap.as_str());
                debug!("permission: {:?}", perm);
                debug!("filtered_policy_opt: {:?}", filtered_policy_opt);
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
            .filter(|perm| ps.cap_state.generic.check_all(&vec![perm.clone()]).is_ok())
            .collect();
        debug!(
            "list of permissions that need to be evaluated: {:?}",
            final_perms
        );
        if !final_perms.is_empty() {
            Some(final_perms)
        } else {
            None
        }
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

    async fn emit_completed(&self, app_id: String) {
        let app_opt = self.platform_state.app_manager_state.get(&app_id);
        if app_opt.is_none() {
            return;
        }
        let app = app_opt.unwrap();
        let sr = SessionResponse::Completed(Self::to_completed_session(&app));
        AppEvents::emit(
            &self.platform_state,
            LCM_EVENT_ON_SESSION_TRANSITION_COMPLETED,
            &serde_json::to_value(sr).unwrap(),
        )
        .await;
    }

    async fn end_session(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
        debug!("end_session: entry: app_id={}", app_id);
        let app = self.platform_state.app_manager_state.remove(app_id);
        if let Some(app) = app {
            if let Some(timer) = self.timer_map.remove(app_id) {
                timer.cancel();
            }
            let transport = app.initial_session.get_transport();
            if let EffectiveTransport::Bridge(container_id) = transport {
                AppEvents::remove_session(&self.platform_state, app.session_id.clone());
                let request = BridgeProtocolRequest::EndSession(container_id);
                if let Err(e) = self
                    .platform_state
                    .get_client()
                    .send_extn_request(request)
                    .await
                {
                    error!("Error sending event to bridge {:?}", e);
                }
            }
        } else {
            error!("end_session app_id={} Not found", app_id);
            return Err(AppError::NotFound);
        }
        Ok(AppManagerResponse::None)
    }

    async fn get_launch_request(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
        match self.platform_state.app_manager_state.get(app_id) {
            Some(app) => {
                let launch_request = LaunchRequest {
                    app_id: app.initial_session.app.id.clone(),
                    intent: app.initial_session.launch.intent,
                };
                Ok(AppManagerResponse::LaunchRequest(launch_request))
            }
            None => Err(AppError::NotFound),
        }
    }

    async fn set_state(
        &mut self,
        app_id: &str,
        state: LifecycleState,
    ) -> Result<AppManagerResponse, AppError> {
        debug!("set_state: entry: app_id={}, state={:?}", app_id, state);
        let am_state = &self.platform_state.app_manager_state;
        let app = am_state.get(app_id);
        if app.is_none() {
            warn!("appid:{} Not found", app_id);
            return Err(AppError::NotFound);
        }

        let app = app.unwrap();
        let previous_state = app.state;

        if previous_state == state {
            warn!(
                "set_state app_id:{} state:{:?} Already in state",
                app_id, state
            );
            return Err(AppError::UnexpectedState);
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
            &self.platform_state,
            app_id.to_string(),
            event_name,
            &serde_json::to_value(state_change).unwrap(),
        )
        .await;

        if LifecycleState::Unloading == state {
            self.on_unloading(app_id).await.ok();
        }
        Ok(AppManagerResponse::None)
    }

    fn ready_check(&self, app_id: &str) -> AppResponse {
        let app = self.platform_state.app_manager_state.get(app_id);
        if app.is_none() {
            warn!("appid:{} Not found", app_id);
            return Err(AppError::NotFound);
        }
        let app = app.unwrap();
        match app.state {
            LifecycleState::Initializing => Ok(AppManagerResponse::None),
            _ => Err(AppError::UnexpectedState),
        }
    }

    fn finished_check(&self, app_id: &str) -> AppResponse {
        let app = self.platform_state.app_manager_state.get(app_id);
        if app.is_none() {
            warn!("appid:{} Not found", app_id);
            return Err(AppError::NotFound);
        }
        let app = app.unwrap();
        match app.state {
            LifecycleState::Unloading => Ok(AppManagerResponse::None),
            _ => Err(AppError::UnexpectedState),
        }
    }

    async fn send_lifecycle_mgmt_event(
        &mut self,
        event: LifecycleManagementEventRequest,
    ) -> Result<AppManagerResponse, AppError> {
        if self.platform_state.has_internal_launcher() {
            if let Err(e) = self.platform_state.get_client().send_event(event) {
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

            AppEvents::emit(&self.platform_state, event_name, &value).await;
        }

        Ok(AppManagerResponse::None)
    }

    async fn get_start_page(&mut self, app_id: String) -> Result<AppManagerResponse, AppError> {
        match self.platform_state.app_manager_state.get(&app_id) {
            Some(app) => Ok(AppManagerResponse::StartPage(app.initial_session.app.url)),
            None => Err(AppError::NotFound),
        }
    }

    async fn start_timer(helper: RippleClient, timeout_ms: u64, method: AppMethod) -> Timer {
        let cb = async move {
            let (resp_tx, resp_rx) = oneshot::channel::<Result<AppManagerResponse, AppError>>();
            let req = AppRequest::new(method, resp_tx);
            if let Err(e) = helper.send_app_request(req) {
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
            None => Err(AppError::NotFound),
        }
    }
}
