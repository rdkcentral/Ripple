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
    api::{
        account_link::{AccountLinkRequest, WatchedRequest},
        apps::{AppManagerResponse, AppMethod, AppRequest, AppResponse},
        distributor::{
            distributor_discovery::{DiscoveryRequest, MediaEventRequest},
            distributor_privacy::DataEventType,
        },
        firebolt::fb_discovery::{
            ClearContentSetParams, ContentAccessAvailability, ContentAccessEntitlement,
            ContentAccessInfo, ContentAccessListSetParams, ContentAccessRequest, MediaEvent,
            MediaEventsAccountLinkRequestParams, SessionParams, SignInRequestParams,
        },
        gateway::rpc_gateway_api::CallContext,
    },
    async_trait::async_trait,
    extn::{
        client::extn_processor::{
            DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
        },
        extn_client_message::{ExtnMessage, ExtnResponse},
    },
    log::{debug, error},
    tokio::sync::{
        mpsc::{Receiver as MReceiver, Sender as MSender},
        oneshot,
    },
    utils::error::RippleError,
};

use crate::{
    firebolt::handlers::discovery_rpc::DiscoveryImpl, service::data_governance::DataGovernance,
    state::platform_state::PlatformState, utils::rpc_utils::rpc_await_oneshot,
};

/// Supports processing of [Config] request from extensions and also
/// internal services.
#[derive(Debug)]
pub struct AccountLinkProcessor {
    state: PlatformState,
    streamer: DefaultExtnStreamer,
}

impl AccountLinkProcessor {
    pub fn new(state: PlatformState) -> AccountLinkProcessor {
        AccountLinkProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    async fn process_sign_in_request(
        state: &PlatformState,
        msg: ExtnMessage,
        ctx: CallContext,
        is_signed_in: bool,
    ) -> bool {
        let session = match state.session_state.get_account_session() {
            Some(session) => session,
            None => {
                error!("No account session found");
                return false;
            }
        };

        let payload = DiscoveryRequest::SignIn(SignInRequestParams {
            session_info: SessionParams {
                app_id: ctx.app_id.to_owned(),
                dist_session: session,
            },
            is_signed_in,
        });

        let resp = state.get_client().send_extn_request(payload).await;

        if let Ok(v) = resp {
            if let Some(ExtnResponse::None(())) = v.payload.extract() {
                return Self::respond(
                    state.get_client().get_extn_client(),
                    msg,
                    ExtnResponse::None(()),
                )
                .await
                .is_ok();
            }
        }

        Self::handle_error(
            state.get_client().get_extn_client(),
            msg,
            RippleError::ProcessorError,
        )
        .await
    }

    async fn content_access(
        state: &PlatformState,
        msg: ExtnMessage,
        ctx: CallContext,
        request: ContentAccessRequest,
    ) -> bool {
        let session = match state.session_state.get_account_session() {
            Some(session) => session,
            None => {
                error!("No account session found");
                return false;
            }
        };

        // If both entitlement & availability are None return EmptyResult
        if request.ids.availabilities.is_none() && request.ids.entitlements.is_none() {
            return Self::respond(
                state.get_client().get_extn_client(),
                msg,
                ExtnResponse::None(()),
            )
            .await
            .is_ok();
        }

        let payload = DiscoveryRequest::SetContentAccess(ContentAccessListSetParams {
            session_info: SessionParams {
                app_id: ctx.app_id.to_owned(),
                dist_session: session,
            },
            content_access_info: ContentAccessInfo {
                availabilities: request.ids.availabilities.map(|availability_vec| {
                    availability_vec
                        .into_iter()
                        .map(|x| ContentAccessAvailability {
                            _type: x._type.as_string().to_owned(),
                            id: x.id,
                            catalog_id: x.catalog_id,
                            start_time: x.start_time,
                            end_time: x.end_time,
                        })
                        .collect()
                }),
                entitlements: request.ids.entitlements.map(|entitlement_vec| {
                    entitlement_vec
                        .into_iter()
                        .map(|x| ContentAccessEntitlement {
                            entitlement_id: x.entitlement_id,
                            start_time: x.start_time,
                            end_time: x.end_time,
                        })
                        .collect()
                }),
            },
        });

        let resp = state.get_client().send_extn_request(payload).await;
        if let Ok(v) = resp {
            if let Some(ExtnResponse::None(())) = v.payload.extract() {
                return Self::respond(
                    state.get_client().get_extn_client(),
                    msg,
                    ExtnResponse::None(()),
                )
                .await
                .is_ok();
            }
        }

        Self::handle_error(
            state.get_client().get_extn_client(),
            msg,
            RippleError::ProcessorError,
        )
        .await
    }

    async fn clear_content_access(
        state: &PlatformState,
        msg: ExtnMessage,
        ctx: CallContext,
    ) -> bool {
        let session = match state.session_state.get_account_session() {
            Some(session) => session,
            None => {
                error!("No account session found");
                return false;
            }
        };

        let payload = DiscoveryRequest::ClearContent(ClearContentSetParams {
            session_info: SessionParams {
                app_id: ctx.app_id.to_owned(),
                dist_session: session,
            },
        });

        let resp = state.get_client().send_extn_request(payload).await;
        if let Ok(v) = resp {
            if let Some(ExtnResponse::None(())) = v.payload.extract() {
                return Self::respond(
                    state.get_client().get_extn_client(),
                    msg,
                    ExtnResponse::None(()),
                )
                .await
                .is_ok();
            }
        }

        Self::handle_error(
            state.get_client().get_extn_client(),
            msg,
            RippleError::ProcessorError,
        )
        .await
    }

    pub async fn get_content_partner_id(
        platform_state: &PlatformState,
        ctx: &CallContext,
    ) -> Result<String, RippleError> {
        let mut content_partner_id = ctx.app_id.to_owned();
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();

        let app_request = AppRequest::new(
            AppMethod::GetAppContentCatalog(ctx.app_id.clone()),
            app_resp_tx,
        );
        if let Err(e) = platform_state.get_client().send_app_request(app_request) {
            error!("Send error for AppMethod::GetAppContentCatalog {:?}", e);
            return Err(RippleError::ProcessorError);
        }
        let resp = rpc_await_oneshot(app_resp_rx).await;

        if let Ok(Ok(AppManagerResponse::AppContentCatalog(content_catalog))) = resp {
            content_partner_id = content_catalog.map_or(ctx.app_id.to_owned(), |x| x)
        }
        Ok(content_partner_id)
    }

    async fn watched(state: &PlatformState, msg: ExtnMessage, request: WatchedRequest) -> bool {
        let watched_info = request.info;
        let ctx = request.context;
        let (data_tags, drop_data) =
            DataGovernance::resolve_tags(state, ctx.app_id.clone(), DataEventType::Watched).await;
        debug!("drop_all={:?} data_tags={:?}", drop_data, data_tags);
        if drop_data {
            return Self::respond(
                state.get_client().get_extn_client(),
                msg,
                ExtnResponse::Boolean(false),
            )
            .await
            .is_ok();
        }

        if let Some(dist_session) = state.session_state.get_account_session() {
            let request =
                MediaEventRequest::MediaEventAccountLink(MediaEventsAccountLinkRequestParams {
                    media_event: MediaEvent {
                        content_id: watched_info.entity_id.to_owned(),
                        completed: watched_info.completed.unwrap_or(true),
                        progress: watched_info.progress,
                        progress_unit: request.unit.clone(),
                        watched_on: watched_info.watched_on.clone(),
                        app_id: ctx.app_id.to_owned(),
                    },
                    content_partner_id: Self::get_content_partner_id(state, &ctx)
                        .await
                        .unwrap_or_else(|_| ctx.app_id.to_owned()),
                    client_supports_opt_out: DiscoveryImpl::get_share_watch_history(),
                    dist_session,
                    data_tags,
                });

            if state.get_client().send_extn_request(request).await.is_ok() {
                return Self::respond(
                    state.get_client().get_extn_client(),
                    msg,
                    ExtnResponse::Boolean(true),
                )
                .await
                .is_ok();
            }
        }
        Self::handle_error(
            state.get_client().get_extn_client(),
            msg,
            RippleError::ProcessorError,
        )
        .await
    }
}

impl ExtnStreamProcessor for AccountLinkProcessor {
    type STATE = PlatformState;
    type VALUE = AccountLinkRequest;
    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn sender(&self) -> MSender<ExtnMessage> {
        self.streamer.sender()
    }

    fn receiver(&mut self) -> MReceiver<ExtnMessage> {
        self.streamer.receiver()
    }
}

#[async_trait]
impl ExtnRequestProcessor for AccountLinkProcessor {
    fn get_client(&self) -> ripple_sdk::extn::client::extn_client::ExtnClient {
        self.state.get_client().get_extn_client()
    }

    async fn process_request(
        state: Self::STATE,
        msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            AccountLinkRequest::SignIn(ctx) => {
                Self::process_sign_in_request(&state, msg, ctx, true).await
            }
            AccountLinkRequest::SignOut(ctx) => {
                Self::process_sign_in_request(&state, msg, ctx, false).await
            }
            AccountLinkRequest::ContentAccess(ctx, request) => {
                Self::content_access(&state, msg, ctx, request).await
            }
            AccountLinkRequest::ClearContentAccess(ctx) => {
                Self::clear_content_access(&state, msg, ctx).await
            }
            AccountLinkRequest::Watched(request) => Self::watched(&state, msg, request).await,
        }
    }
}
