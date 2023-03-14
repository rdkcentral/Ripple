use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use ripple_sdk::{
    api::{
        apps::AppRequest,
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
    tokio::{
        self,
        sync::mpsc::Receiver,
        time::{sleep, Duration},
    },
};
use ripple_sdk::{
    api::{
        apps::{
            AppError, AppManagerResponse, AppMethod, AppSession, EffectiveTransport, StateChange,
        },
        firebolt::{
            fb_discovery::DISCOVERY_EVENT_ON_NAVIGATE_TO, fb_lifecycle::LifecycleState,
            fb_secondscreen::SECOND_SCREEN_EVENT_ON_LAUNCH_REQUEST,
        },
    },
    log::{debug, error, warn},
    serde_json::{self},
    uuid::Uuid,
};

use crate::{
    service::apps::app_events::AppEvents,
    state::{
        bootstrap_state::ChannelsState, cap::permitted_state::PermissionHandler,
        platform_state::PlatformState,
    },
};

#[derive(Debug, Clone)]
pub struct App {
    pub initial_session: AppSession,
    pub current_session: AppSession,
    pub session_id: String,
    pub state: LifecycleState,
}

#[derive(Debug, Clone, Default)]
pub struct AppManagerState {
    apps: Arc<RwLock<HashMap<String, App>>>,
}

impl AppManagerState {
    pub fn exists(&self, app_id: &str) -> bool {
        self.apps.read().unwrap().contains_key(app_id)
    }

    fn set_session(&self, app_id: &str, session: AppSession) {
        let mut apps = self.apps.write().unwrap();
        if let Some(app) = apps.get_mut(app_id) {
            app.current_session = session
        }
    }

    fn set_state(&self, app_id: &str, state: LifecycleState) {
        let mut apps = self.apps.write().unwrap();
        if let Some(app) = apps.get_mut(app_id) {
            app.state = state;
        }
    }

    fn get_session_id(&self, app_id: &str) -> Option<String> {
        if let Some(app) = self.apps.read().unwrap().get(app_id) {
            return Some(app.session_id.clone());
        }
        None
    }

    fn insert(&self, app_id: String, app: App) {
        let mut apps = self.apps.write().unwrap();
        let _ = apps.insert(app_id, app);
    }

    fn get(&self, app_id: &str) -> Option<App> {
        self.apps.read().unwrap().get(app_id).cloned()
    }

    fn remove(&self, app_id: &str) -> Option<App> {
        let mut apps = self.apps.write().unwrap();
        apps.remove(app_id)
    }
}

pub struct DelegatedLauncherHandler {
    platform_state: PlatformState,
    app_mgr_req_rx: Receiver<AppRequest>,
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
                    resp = self
                        .send_lifecycle_mgmt_event(LifecycleManagementEventRequest::Launch(
                            LifecycleManagementLaunchEvent {
                                parameters: LifecycleManagementLaunchParameters {
                                    app_id: launch_request.app_id.clone(),
                                    intent: Some(launch_request.get_intent()),
                                },
                            },
                        ))
                        .await;
                }
                AppMethod::Ready(app_id) => {
                    resp = self
                        .send_lifecycle_mgmt_event(LifecycleManagementEventRequest::Ready(
                            LifecycleManagementReadyEvent {
                                parameters: LifecycleManagementReadyParameters { app_id: app_id },
                            },
                        ))
                        .await;
                }
                AppMethod::Close(app_id, reason) => {
                    resp = self
                        .send_lifecycle_mgmt_event(LifecycleManagementEventRequest::Close(
                            LifecycleManagementCloseEvent {
                                parameters: LifecycleManagementCloseParameters {
                                    app_id: app_id,
                                    reason: reason,
                                },
                            },
                        ))
                        .await;
                }
                AppMethod::CheckFinished(app_id) => {
                    resp = self.check_finished(&app_id).await;
                }
                AppMethod::Finished(app_id) => {
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

    async fn start_session(&mut self, session: AppSession) -> Result<AppManagerResponse, AppError> {
        let app_id = session.app.id.clone();
        debug!("start_session: entry: app_id={}", app_id);
        let exists = self.platform_state.app_manager_state.exists(&app_id);
        let session_id;
        if exists {
            self.platform_state
                .app_manager_state
                .set_session(&app_id, session.clone());
            AppEvents::emit(
                &self.platform_state,
                DISCOVERY_EVENT_ON_NAVIGATE_TO,
                &serde_json::to_value(session.launch.intent).unwrap(),
            )
            .await;

            if let Some(ss) = session.launch.second_screen {
                AppEvents::emit(
                    &self.platform_state,
                    SECOND_SCREEN_EVENT_ON_LAUNCH_REQUEST,
                    &serde_json::to_value(ss).unwrap(),
                )
                .await;
            }

            if let Some(v) = self
                .platform_state
                .app_manager_state
                .get_session_id(&app_id)
            {
                session_id = v;
            } else {
                return Err(AppError::AppNotReady);
            }
        } else {
            session_id = Uuid::new_v4().to_string();
            let platform_state_c = self.platform_state.clone();
            let app_id_c = app_id.clone();

            // Fetch permissions on separate thread
            tokio::spawn(async move {
                if let Err(_) =
                    PermissionHandler::fetch_and_store(platform_state_c, app_id_c.clone()).await
                {
                    error!("Couldnt load permissions for app {}", app_id_c)
                }
            });

            // TODO add bridge logic for Effective Transport
            self.platform_state.app_manager_state.insert(
                app_id,
                App {
                    initial_session: session.clone(),
                    current_session: session.clone(),
                    session_id: session_id.clone(),
                    state: LifecycleState::Initializing,
                },
            );
        }
        return Ok(AppManagerResponse::SessionId(session_id));
    }

    async fn end_session(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
        debug!("end_session: entry: app_id={}", app_id);
        let app = self.platform_state.app_manager_state.remove(app_id);
        if let Some(app) = app {
            let transport = app.initial_session.get_transport();
            if let EffectiveTransport::Bridge(_browser_name) = transport {
                // TODO add support for bridge sesssion closure
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
                    intent: Some(app.initial_session.launch.intent.clone()),
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
        let app = self.platform_state.app_manager_state.get(app_id);
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
        self.platform_state
            .app_manager_state
            .set_state(app_id, state);
        let state_change = StateChange {
            state: state.clone(),
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
            self.on_unloading(&app_id).await.ok();
        }
        Ok(AppManagerResponse::None)
    }

    async fn send_lifecycle_mgmt_event(
        &mut self,
        event: LifecycleManagementEventRequest,
    ) -> Result<AppManagerResponse, AppError> {
        if self.platform_state.has_internal_launcher() {
            if let Err(e) = self.platform_state.get_client().send_event(event).await {
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
            }

            AppEvents::emit(&self.platform_state, event_name, &value).await;
        }

        Ok(AppManagerResponse::None)
    }

    async fn get_start_page(&mut self, app_id: String) -> Result<AppManagerResponse, AppError> {
        match self.platform_state.app_manager_state.get(&app_id) {
            Some(app) => Ok(AppManagerResponse::StartPage(
                app.initial_session.app.url.clone(),
            )),
            None => Err(AppError::NotFound),
        }
    }

    async fn on_unloading(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
        debug!("on_unloading: entry: app_id={}", app_id);
        let timer_ms = self
            .platform_state
            .get_device_manifest()
            .get_lifecycle_policy()
            .app_finished_timeout_ms;
        sleep(Duration::from_millis(timer_ms)).await;
        self.check_finished(app_id).await
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
                return self.end_session(app_id).await;
            }
            None => {
                return Ok(AppManagerResponse::None);
            }
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
                app.initial_session.app.catalog.clone(),
            )),
            None => Err(AppError::NotFound),
        }
    }
}
