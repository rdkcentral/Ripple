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

use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::{
    api::{
        apps::{AppManagerResponse, AppMethod, AppRequest, AppResponse},
        device::entertainment_data::NavigationIntent,
        gateway::rpc_gateway_api::CallContext,
    },
    tokio::sync::oneshot,
};
use serde::{Deserialize, Serialize};

use crate::{
    firebolt::rpc::RippleRPCProvider, state::platform_state::PlatformState,
    utils::rpc_utils::rpc_await_oneshot,
};

use super::privacy_rpc;

#[derive(Serialize, Debug, Clone)]
pub struct AppInitParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub us_privacy: Option<String>,
    pub lmt: Option<u16>,
    pub discovery: Option<DiscoveryEvent>,
    #[serde(rename = "secondScreen", skip_serializing_if = "Option::is_none")]
    pub second_screen: Option<SecondScreenEvent>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SecondScreenEvent {
    #[serde(rename = "type")]
    pub _type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct DiscoveryEvent {
    #[serde(rename = "navigateTo")]
    pub navigate_to: NavigationIntent,
}

#[rpc(server)]
pub trait Parameters {
    #[method(name = "parameters.initialization")]
    async fn initialization(&self, _ctx: CallContext) -> RpcResult<AppInitParameters>;
}

pub struct ParametersImpl {
    platform_state: PlatformState,
}

#[async_trait]
impl ParametersServer for ParametersImpl {
    async fn initialization(&self, ctx: CallContext) -> RpcResult<AppInitParameters> {
        let (app_resp_tx, app_resp_rx) = oneshot::channel::<AppResponse>();
        let app_request = AppRequest::new(
            AppMethod::GetLaunchRequest(ctx.app_id.to_owned()),
            app_resp_tx,
        );
        let privacy_data = privacy_rpc::get_allow_app_content_ad_targeting_settings(
            &self.platform_state,
            None,
            &ctx.app_id.to_string(),
            &ctx,
        )
        .await;

        let _ = self
            .platform_state
            .get_client()
            .send_app_request(app_request);
        let resp = rpc_await_oneshot(app_resp_rx).await?;
        if let AppManagerResponse::LaunchRequest(launch_req) = resp? {
            return Ok(AppInitParameters {
                us_privacy: privacy_data.get(privacy_rpc::US_PRIVACY_KEY).cloned(),
                lmt: privacy_data
                    .get(privacy_rpc::LMT_KEY)
                    .and_then(|x| x.parse::<u16>().ok()),
                discovery: Some(DiscoveryEvent {
                    navigate_to: launch_req.get_intent(),
                }),
                second_screen: None,
            });
        }

        Err(jsonrpsee::core::Error::Custom(String::from(
            "Internal Error",
        )))
    }
}

pub struct ParametersRPCProvider;

impl RippleRPCProvider<ParametersImpl> for ParametersRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<ParametersImpl> {
        (ParametersImpl {
            platform_state: state,
        })
        .into_rpc()
    }
}
