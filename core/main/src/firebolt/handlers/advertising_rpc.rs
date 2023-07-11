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
    core::{async_trait, Error, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_advertising::{
                AdIdRequestParams, AdInitObjectRequestParams, AdvertisingFrameworkConfig,
                AdvertisingRequest, AdvertisingResponse, GetAdConfig,
            },
            fb_capabilities::{CapabilityRole, RoleInfo},
            fb_general::{ListenRequest, ListenerResponse},
        },
        gateway::rpc_gateway_api::CallContext,
        storage_property::EVENT_ADVERTISING_POLICY_CHANGED,
    },
    extn::extn_client_message::ExtnResponse,
    log::error,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::{
    firebolt::rpc::RippleRPCProvider,
    state::platform_state::PlatformState,
    utils::rpc_utils::{rpc_add_event_listener, rpc_err},
};

use super::{
    capabilities_rpc::is_permitted,
    privacy_rpc::{self, PrivacyImpl},
};

const ADVERTISING_APP_BUNDLE_ID_SUFFIX: &'static str = "Comcast";

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AdvertisingId {
    pub ifa: String,
    pub ifa_type: String,
    pub lmt: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvertisingPolicy {
    pub skip_restriction: String,
    pub limit_ad_tracking: bool,
}

#[rpc(server)]
pub trait Advertising {
    #[method(name = "advertising.advertisingId")]
    async fn advertising_id(&self, ctx: CallContext) -> RpcResult<AdvertisingId>;
    #[method(name = "advertising.appBundleId")]
    fn app_bundle_id(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "advertising.config")]
    async fn config(
        &self,
        ctx: CallContext,
        config: GetAdConfig,
    ) -> RpcResult<AdvertisingFrameworkConfig>;
    #[method(name = "advertising.deviceAttributes")]
    async fn device_attributes(&self, ctx: CallContext) -> RpcResult<Value>;
    #[method(name = "advertising.policy")]
    async fn policy(&self, ctx: CallContext) -> RpcResult<AdvertisingPolicy>;
    #[method(name = "advertising.onPolicyChanged")]
    async fn advertising_on_policy_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "advertising.resetIdentifier")]
    async fn reset_identifier(&self, ctx: CallContext) -> RpcResult<()>;
}

pub struct AdvertisingImpl {
    pub state: PlatformState,
}

#[async_trait]
impl AdvertisingServer for AdvertisingImpl {
    async fn reset_identifier(&self, _ctx: CallContext) -> RpcResult<()> {
        let session = self.state.session_state.get_account_session().unwrap();

        let resp = self
            .state
            .get_client()
            .send_extn_request(AdvertisingRequest::ResetAdIdentifier(session))
            .await;

        if let Err(_) = resp {
            error!("Error resetting ad identifier {:?}", resp);
            return Err(rpc_err("Could not reset ad identifier for the device"));
        }

        match resp {
            Ok(payload) => match payload.payload.extract().unwrap() {
                ExtnResponse::None(()) => Ok(()),
                _ => Err(rpc_err("Device returned Unable to reset ad identifier")),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Unable to reset ad identifier",
            ))),
        }
    }

    async fn advertising_id(&self, ctx: CallContext) -> RpcResult<AdvertisingId> {
        let session = self.state.session_state.get_account_session().unwrap();
        let payload = AdvertisingRequest::GetAdIdObject(AdIdRequestParams {
            privacy_data: privacy_rpc::get_allow_app_content_ad_targeting_settings(&self.state)
                .await,
            app_id: ctx.clone().app_id.to_string(),
            dist_session: session,
        });
        let resp = self.state.get_client().send_extn_request(payload).await;

        if let Err(_) = resp {
            error!("Error getting ad init object: {:?}", resp);
            return Err(rpc_err("Could not get ad init object from the device"));
        }

        match resp {
            Ok(payload) => match payload.payload.extract::<AdvertisingResponse>() {
                Some(response) => {
                    if let AdvertisingResponse::AdIdObject(obj) = response {
                        let ad_init_object = AdvertisingId {
                            ifa: obj.ifa,
                            ifa_type: obj.ifa_type,
                            lmt: obj.lmt,
                        };
                        return Ok(ad_init_object);
                    }
                }
                None => {}
            },
            Err(_) => {}
        }
        Err(jsonrpsee::core::Error::Custom(String::from(
            "Failed to extract ad init object from response",
        )))
    }

    fn app_bundle_id(&self, ctx: CallContext) -> RpcResult<String> {
        Ok(format!(
            "{}.{}",
            ctx.app_id, ADVERTISING_APP_BUNDLE_ID_SUFFIX
        ))
    }

    async fn config(
        &self,
        ctx: CallContext,
        config: GetAdConfig,
    ) -> RpcResult<AdvertisingFrameworkConfig> {
        let session = self.state.session_state.get_account_session();
        let app_id = ctx.app_id.to_string();
        let distributor_experience_id = self
            .state
            .get_device_manifest()
            .get_distributor_experience_id();
        let params = RoleInfo {
            capability: "advertising:identifier".to_string(),
            role: Some(CapabilityRole::Use),
        };
        let ad_id_authorised = is_permitted(self.state.clone(), ctx, params).await;

        let payload = AdvertisingRequest::GetAdInitObject(AdInitObjectRequestParams {
            privacy_data: privacy_rpc::get_allow_app_content_ad_targeting_settings(&self.state)
                .await,
            environment: config.options.environment.to_string(),
            durable_app_id: app_id,
            app_version: "".to_string(),
            distributor_app_id: distributor_experience_id,
            device_ad_attributes: HashMap::new(),
            coppa: config.options.coppa.unwrap_or(false),
            authentication_entity: config.options.authentication_entity.unwrap_or_default(),
            dist_session: session.unwrap(),
        });

        let resp = self.state.get_client().send_extn_request(payload).await;

        if let Err(_) = resp {
            error!("Error getting ad init object: {:?}", resp);
            return Err(rpc_err("Could not get ad init object from the device"));
        }

        match resp {
            Ok(payload) => match payload.payload.extract().unwrap() {
                AdvertisingResponse::AdInitObject(obj) => {
                    let ad_init_object = AdvertisingFrameworkConfig {
                        ad_server_url: obj.ad_server_url,
                        ad_server_url_template: obj.ad_server_url_template,
                        ad_network_id: obj.ad_network_id,
                        ad_profile_id: obj.ad_profile_id,
                        ad_site_section_id: "".to_string(),
                        ad_opt_out: obj.ad_opt_out,
                        privacy_data: obj.privacy_data,
                        ifa: if ad_id_authorised.unwrap() {
                            obj.ifa
                        } else {
                            "0".repeat(obj.ifa.len())
                        },
                        ifa_value: obj.ifa_value,
                        app_name: obj.app_name,
                        app_bundle_id: obj.app_bundle_id,
                        distributor_app_id: obj.distributor_app_id,
                        device_ad_attributes: obj.device_ad_attributes,
                        coppa: obj.coppa.to_string().parse::<u32>().unwrap(),
                        authentication_entity: obj.authentication_entity,
                    };
                    Ok(ad_init_object)
                }
                _ => Err(rpc_err(
                    "Device returned an invalid type for ad init object",
                )),
            },
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "Failed to extract ad init object from response",
            ))),
        }
    }

    async fn device_attributes(&self, ctx: CallContext) -> RpcResult<Value> {
        let afc = self.config(ctx.clone(), Default::default()).await?;

        let buff = base64::decode(afc.device_ad_attributes).unwrap_or_default();
        match String::from_utf8(buff) {
            Ok(mut b_string) => {
                /*
                dealing with: https://ccp.sys.comcast.net/browse/OTTX-28561
                This is actually caused by ad-platform-service getting a 404
                from Freewheel and returning an error.
                */

                if b_string.trim().len() == 0 {
                    b_string = "{}".to_string();
                };

                match serde_json::from_str(b_string.as_str()) {
                    Ok(js) => Ok(js),
                    Err(_e) => Err(Error::Custom(String::from("Invalid JSON"))),
                }
            }
            Err(_e) => Err(Error::Custom(String::from("Found invalid UTF-8"))),
        }
    }

    async fn policy(&self, _ctx: CallContext) -> RpcResult<AdvertisingPolicy> {
        let limit_ad_tracking = PrivacyImpl::get_allow_app_content_ad_targeting(&self.state).await;
        Ok(AdvertisingPolicy {
            skip_restriction: "adsUnwatched".to_owned(),
            limit_ad_tracking,
        })
    }

    async fn advertising_on_policy_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener(&self.state, ctx, request, EVENT_ADVERTISING_POLICY_CHANGED).await
    }
}

pub struct AdvertisingRPCProvider;
impl RippleRPCProvider<AdvertisingImpl> for AdvertisingRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<AdvertisingImpl> {
        (AdvertisingImpl { state }).into_rpc()
    }
}
