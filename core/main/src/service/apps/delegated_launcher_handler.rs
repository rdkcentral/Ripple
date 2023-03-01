use std::{
    collections::HashMap,
    os::macos::raw::stat,
    sync::{Arc, RwLock},
};

use jsonrpsee::core::async_trait;
use ripple_sdk::tokio::{
    self,
    sync::{
        mpsc::{Receiver, Sender},
        oneshot,
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
    log::{debug, warn},
    serde_json::{self, Value},
    uuid::Uuid,
};

use crate::{
    service::apps::app_events::AppEvents,
    state::{bootstrap_state::ChannelsState, platform_state::PlatformState},
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
    fn exists(&self, app_id: &str) -> bool {
        self.apps.read().unwrap().contains_key(app_id)
    }

    fn set_session(&self, app_id: &str, session: AppSession) {
        let mut apps = self.apps.write().unwrap();
        if let Some(app) = apps.get_mut(app_id) {
            app.current_session = session
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
}

pub struct DelegatedLauncherHandler;

impl DelegatedLauncherHandler {
    pub async fn start(channels_state: ChannelsState, platform_state: PlatformState) {
        let receiver = channels_state
            .get_app_mgr_receiver()
            .expect("App Mgr receiver to be available");
        tokio::spawn(async move {
            tokio::select! {
                data = receiver.recv() => {
                    // App request
                    debug!("DelegatedLauncherHandler: App request: data={:?}", data);
                    match data {
                        Some(req) => {
                            let resp;
                            match req.method {
                                AppMethod::BrowserSession(session) => {
                                    resp = start_session(platform_state.clone(), session).await;
                                }
                                AppMethod::SetState(app_id, state) => {
                                    resp = set_state(platform_state.clone(), &app_id, state).await;
                                }
                                AppMethod::Launch(launch_request) => {
                                    resp = self.send_lifecycle_mgmt_event(LifecycleManagementRequest::Launch(
                                        LifecycleManagementLaunchRequest {
                                            parameters: LifecycleManagementLaunchParameters {
                                                app_id: launch_request.app_id.clone(),
                                                intent: Some(launch_request.get_intent()),
                                            }
                                        })).await;
                                }
                                AppMethod::Ready(app_id) => {
                                    resp = self.send_lifecycle_mgmt_event(LifecycleManagementRequest::Ready(
                                        LifecycleManagementReadyRequest {
                                                parameters: LifecycleManagementReadyParameters {
                                                    app_id: app_id,
                                                }
                                            }
                                        )).await;
                                }
                                AppMethod::Close(app_id, reason) => {
                                    resp = self.send_lifecycle_mgmt_event(LifecycleManagementRequest::Close(
                                        LifecycleManagementCloseRequest {
                                                parameters: LifecycleManagementCloseParameters {
                                                    app_id: app_id,
                                                    reason: reason,
                                                }
                                            }
                                    )).await;
                                }
                                AppMethod::CheckFinished(app_id) => {
                                    resp = self.check_finished(&app_id).await;
                                }
                                AppMethod::Finished(app_id) => {
                                    self.send_lifecycle_mgmt_event(LifecycleManagementRequest::Finished(
                                        LifecycleManagementFinishedRequest {
                                                parameters: LifecycleManagementFinishedParameters {
                                                    app_id: app_id.clone(),
                                                }
                                            }
                                    )).await.ok();
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
                                _ => resp = Err(AppError::NotSupported)
                            }
                            if let Err(e) = resp {
                                error!(?e, "App error");
                            }
                            if let Some(resp_tx) = req.resp_tx {
                                oneshot_send_and_log(resp_tx, resp, "AppManager response");
                            }
                        },
                        None => {
                            return false;
                        }
                    }
                }
            }
        });
    }
}

async fn start_session(
    state: PlatformState,
    session: AppSession,
) -> Result<AppManagerResponse, AppError> {
    let app_id = session.app.id.clone();
    debug!("start_session: entry: app_id={}", app_id);
    let transport = session.get_transport();
    let exists = state.app_mgr_state.exists(&app_id);
    let session_id;
    if exists {
        state.app_mgr_state.set_session(&app_id, session.clone());
        AppEvents::emit(
            &state,
            DISCOVERY_EVENT_ON_NAVIGATE_TO,
            &serde_json::to_value(session.launch.intent).unwrap(),
        )
        .await;

        if let Some(ss) = session.launch.second_screen {
            AppEvents::emit(
                &state,
                SECOND_SCREEN_EVENT_ON_LAUNCH_REQUEST,
                &serde_json::to_value(ss).unwrap(),
            )
            .await;
        }

        if let Some(v) = state.app_mgr_state.get_session_id(&app_id) {
            session_id = v;
        } else {
            return Err(AppError::AppNotReady);
        }
    } else {
        session_id = Uuid::new_v4().to_string(); // TODO add bridge logic for Effective Transport
        state.app_mgr_state.insert(
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
    let app = self.apps.remove(app_id);
    AppManager::remove_session_by_app_id(
        &&self.platform_state.app_sessions_state,
        &String::from(app_id),
    );
    if let Some(app) = app {
        let transport = app.initial_session.get_transport();
        if let EffectiveTransport::Bridge(browser_name) = transport {
            FireboltGateway::close_dab_api_session(
                self.firebolt_gateway_tx.clone(),
                self.helper_factory.clone(),
                app.session_id.clone(),
                browser_name,
            )
            .await;
        }
    } else {
        error!(%app_id, "end_session: Not found");
        return Err(AppError::NotFound);
    }
    Ok(AppManagerResponse::None)
}

async fn get_launch_request(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
    match self.apps.get(app_id) {
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
    platform_state: PlatformState,
    app_id: &str,
    state: LifecycleState,
) -> Result<AppManagerResponse, AppError> {
    debug!("set_state: entry: app_id={}, state={:?}", app_id, state);
    let app_mgr_state = platform_state.clone().app_mgr_state;
    let app = app_mgr_state.get(app_id);
    if app.is_none() {
        warn!("appid:{} Not found", app_id);
        return Err(AppError::NotFound);
    }

    let mut app = app.unwrap();
    let previous_state = app.state;

    if previous_state == state {
        warn!(
            "set_state app_id:{} state:{:?} Already in state",
            app_id, state
        );
        return Err(AppError::UnexpectedState);
    }

    app.state = state;
    let state_change = StateChange {
        state: state.clone(),
        previous: previous_state,
    };
    let event_name = state.as_event();
    AppEvents::emit_to_app(
        &platform_state,
        app_id.to_string(),
        event_name,
        &serde_json::to_value(state_change).unwrap(),
    )
    .await;

    if LifecycleState::Unloading == state {
        on_unloading(platform_state.clone(), &app_id).await.ok();
    }
    Ok(AppManagerResponse::None)
}

async fn send_lifecycle_mgmt_event(
    &mut self,
    request: LifecycleManagementRequest,
) -> Result<AppManagerResponse, AppError> {
    let event_name;
    let value;
    match request {
        LifecycleManagementRequest::Launch(req) => {
            event_name = EVENT_ON_REQUEST_LAUNCH;
            value = serde_json::to_value(req).unwrap();
        }
        LifecycleManagementRequest::Ready(req) => {
            event_name = EVENT_ON_REQUEST_READY;
            value = serde_json::to_value(req).unwrap();
        }
        LifecycleManagementRequest::Close(req) => {
            event_name = EVENT_ON_REQUEST_CLOSE;
            value = serde_json::to_value(req).unwrap();
        }
        LifecycleManagementRequest::Finished(req) => {
            event_name = EVENT_ON_REQUEST_FINISHED;
            value = serde_json::to_value(req).unwrap();
        }
    }

    AppEvents::emit(&self.platform_state, event_name, &value).await;

    Ok(AppManagerResponse::None)
}

async fn get_start_page(&mut self, app_id: String) -> Result<AppManagerResponse, AppError> {
    match self.apps.get(&app_id) {
        Some(app) => Ok(AppManagerResponse::StartPage(
            app.initial_session.app.url.clone(),
        )),
        None => Err(AppError::NotFound),
    }
}

async fn start_timer(&mut self, timeout_ms: u64, method: AppMethod) -> Result<(), ()> {
    let (timer_resp_tx, timer_resp_rx) = oneshot::channel::<Result<(), ()>>();
    let timeout_message = AppRequest {
        method,
        resp_tx: None,
    };
    let timer_request = TimerRequest {
        message: TimerMessage::StartTimer(timeout_ms, timeout_message),
        resp_tx: Some(timer_resp_tx),
    };
    if let Err(e) = self
        .timer_req_tx
        .as_ref()
        .unwrap()
        .send(timer_request)
        .await
    {
        error!(?e, "start_timer: Send error");
        Err(())
    } else {
        match timer_resp_rx.await {
            Ok(resp) => match resp {
                Ok(_) => Ok(()),
                Err(()) => Err(()),
            },
            Err(e) => {
                error!(?e, "start_timer: Receive error");
                Err(())
            }
        }
    }
}

async fn on_unloading(
    platform_state: PlatformState,
    app_id: &str,
) -> Result<AppManagerResponse, AppError> {
    debug!("on_unloading: entry: app_id={}", app_id);
    let id = app_id.to_string();
    if let Err(_) = self
        .start_timer(
            self.helper
                .get_config()
                .get_lifecycle_policy()
                .app_finished_timeout_ms,
            AppMethod::CheckFinished(id),
        )
        .await
    {
        error!("on_unloading: Could not start unloading timer");
        self.end_session(app_id).await.ok();
        return Err(AppError::OsError);
    }
    Ok(AppManagerResponse::None)
}

async fn check_finished(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
    debug!("check_finished: app_id={}", app_id);
    let entry = self.apps.get(app_id);
    match entry {
        Some(_app) => {
            warn!(%app_id, "check_finished: App not finished unloading, forcing");
            return self.end_session(app_id).await;
        }
        None => {
            return Ok(AppManagerResponse::None);
        }
    }
}

fn get_second_screen_payload(&mut self, app_id: &str) -> Result<AppManagerResponse, AppError> {
    if let Some(app) = self.apps.get(app_id) {
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
    match self.apps.get(&app_id) {
        Some(app) => Ok(AppManagerResponse::AppContentCatalog(
            app.initial_session.app.catalog.clone(),
        )),
        None => Err(AppError::NotFound),
    }
}
