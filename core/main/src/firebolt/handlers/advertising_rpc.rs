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
use crate::{
    firebolt::rpc::RippleRPCProvider,
    processor::storage::storage_manager::StorageManager,
    service::apps::app_events::{AppEventDecorationError, AppEventDecorator, AppEvents},
    state::platform_state::PlatformState,
    utils::rpc_utils::{rpc_add_event_listener_with_decorator, rpc_err},
};
use base64::{engine::general_purpose::STANDARD as base64, Engine};
use jsonrpsee::{
    core::{async_trait, Error, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use ripple_sdk::{
    api::{
        firebolt::{
            fb_advertising::{
                AdConfigRequestParams, AdConfigResponse, AdIdRequestParams,
                AdvertisingFrameworkConfig, AdvertisingRequest, AdvertisingResponse, GetAdConfig,
            },
            fb_capabilities::{CapabilityRole, FireboltCap, RoleInfo},
            fb_general::{ListenRequest, ListenerResponse},
        },
        gateway::rpc_gateway_api::CallContext,
        storage_property::{
            StorageProperty, EVENT_ADVERTISING_POLICY_CHANGED,
            EVENT_ADVERTISING_SKIP_RESTRICTION_CHANGED,
        },
    },
    log::{debug, error},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::{
    capabilities_rpc::is_permitted,
    privacy_rpc::{self, PrivacyImpl},
};

const ADVERTISING_APP_BUNDLE_ID_SUFFIX: &str = "Comcast";
//{"xifa":"00000000-0000-0000-0000-000000000000","xifaType":"sessionId","lmt":"0"}
const IFA_ZERO_BASE64: &str = "eyJ4aWZhIjoiMDAwMDAwMDAtMDAwMC0wMDAwLTAwMDAtMDAwMDAwMDAwMDAwIiwieGlmYVR5cGUiOiJzZXNzaW9uSWQiLCJsbXQiOiIwIn0K";

#[derive(Debug)]
pub struct AdvertisingId {
    pub ifa: String,
    pub ifa_type: String,
    pub lmt: String,
}

impl Serialize for AdvertisingId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serde_json::Map::new();
        map.insert("ifa".to_string(), self.ifa.clone().into());
        // include both ifaType and ifa_type for backward compatibility
        map.insert("ifaType".to_string(), self.ifa_type.clone().into());
        map.insert("ifa_type".to_string(), self.ifa_type.clone().into());
        map.insert("lmt".to_string(), self.lmt.clone().into());
        map.serialize(serializer)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvertisingPolicy {
    pub skip_restriction: String,
    pub limit_ad_tracking: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SkipRestriction {
    None,
    AdsUnwatched,
    AdsAll,
    All,
}

impl SkipRestriction {
    pub fn as_string(&self) -> &'static str {
        match self {
            SkipRestriction::None => NONE,
            SkipRestriction::AdsUnwatched => "adsUnwatched",
            SkipRestriction::AdsAll => "adsAll",
            SkipRestriction::All => "all",
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SetSkipRestrictionRequest {
    pub value: SkipRestriction,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct AdvertisingIdRPCRequest {
    pub options: Option<ScopeOption>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScopeOption {
    pub scope: Option<Scope>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Scope {
    #[serde(rename = "type")]
    pub _type: ScopeType,
    pub id: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ScopeType {
    Browse,
    Content,
}

impl ScopeType {
    pub fn as_string(&self) -> &'static str {
        match self {
            ScopeType::Browse => "browse",
            ScopeType::Content => "content",
        }
    }
}

#[rpc(server)]
pub trait Advertising {
    #[method(name = "advertising.advertisingId")]
    async fn advertising_id(
        &self,
        ctx: CallContext,
        request: Option<AdvertisingIdRPCRequest>,
    ) -> RpcResult<AdvertisingId>;
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
    #[method(name = "advertising.setSkipRestriction")]
    async fn advertising_set_skip_restriction(
        &self,
        ctx: CallContext,
        set_request: SetSkipRestrictionRequest,
    ) -> RpcResult<()>;
    #[method(name = "advertising.skipRestriction")]
    async fn advertising_skip_restriction(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "advertising.onSkipRestrictionChanged")]
    async fn advertising_on_skip_restriction_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "advertising.onPolicyChanged")]
    async fn advertising_on_policy_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "advertising.resetIdentifier")]
    async fn reset_identifier(&self, ctx: CallContext) -> RpcResult<()>;
}
const NONE: &str = "none";
async fn get_advertisting_policy(platform_state: &PlatformState) -> AdvertisingPolicy {
    AdvertisingPolicy {
        skip_restriction: StorageManager::get_string(
            platform_state,
            StorageProperty::SkipRestriction,
        )
        .await
        .unwrap_or_else(|_| String::from(NONE)),
        limit_ad_tracking: !privacy_rpc::PrivacyImpl::get_allow_app_content_ad_targeting(
            platform_state,
        )
        .await,
    }
}
#[derive(Clone)]
struct AdvertisingPolicyEventDecorator {}
#[async_trait]
impl AppEventDecorator for AdvertisingPolicyEventDecorator {
    async fn decorate(
        &self,
        ps: &PlatformState,
        _ctx: &CallContext,
        _event_name: &str,
        _val_in: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        Ok(serde_json::to_value(get_advertisting_policy(ps).await)?)
    }
    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
struct AdvertisingSetRestrictionEventDecorator {}
use std::convert::From;
#[async_trait]
impl AppEventDecorator for AdvertisingSetRestrictionEventDecorator {
    async fn decorate(
        &self,
        ps: &PlatformState,
        _ctx: &CallContext,
        _event_name: &str,
        _val_in: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        Ok(serde_json::to_value(
            StorageManager::get_string(ps, StorageProperty::SkipRestriction)
                .await
                .unwrap_or_else(|_| String::from(NONE)),
        )?)
    }
    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        Box::new(self.clone())
    }
}

pub struct AdvertisingImpl {
    pub state: PlatformState,
}

fn get_scope_option_map(options: &Option<ScopeOption>) -> HashMap<String, String> {
    let mut scope_option_map = HashMap::new();
    if let Some(scope_opt) = options {
        if let Some(scope) = &scope_opt.scope {
            scope_option_map.insert("type".to_string(), scope._type.as_string().to_string());
            scope_option_map.insert("id".to_string(), scope.id.to_string());
        }
    }
    scope_option_map
}

#[async_trait]
impl AdvertisingServer for AdvertisingImpl {
    async fn reset_identifier(&self, _ctx: CallContext) -> RpcResult<()> {
        self.state
            .get_client()
            .send_extn_request(AdvertisingRequest::ResetAdIdentifier(
                self.state
                    .session_state
                    .get_account_session()
                    .ok_or_else(|| Error::Custom(String::from("no session available")))?,
            ))
            .await
            .map(|_ok| ())
            .map_err(|err| err.into())
    }

    async fn advertising_id(
        &self,
        ctx: CallContext,
        request: Option<AdvertisingIdRPCRequest>,
    ) -> RpcResult<AdvertisingId> {
        if let Some(session) = self.state.session_state.get_account_session() {
            let opts = match request {
                Some(r) => r.options,
                None => None,
            };
            let payload = AdvertisingRequest::GetAdIdObject(AdIdRequestParams {
                privacy_data: privacy_rpc::get_allow_app_content_ad_targeting_settings(
                    &self.state,
                    opts.as_ref(),
                    &ctx.app_id,
                    &ctx,
                )
                .await,
                app_id: ctx.app_id.to_owned(),
                dist_session: session,
                scope: get_scope_option_map(&opts),
            });
            let resp = self.state.get_client().send_extn_request(payload).await;

            if resp.is_err() {
                error!("Error getting ad init object: {:?}", resp);
                return Err(rpc_err("Could not get ad init object from the device"));
            }

            if let Ok(payload) = resp {
                if let Some(AdvertisingResponse::AdIdObject(obj)) =
                    payload.payload.extract::<AdvertisingResponse>()
                {
                    let ad_id = AdvertisingId {
                        ifa: obj.ifa,
                        ifa_type: obj.ifa_type,
                        lmt: obj.lmt,
                    };
                    return Ok(ad_id);
                }
            }

            Err(jsonrpsee::core::Error::Custom(String::from(
                "Failed to extract ad init object from response",
            )))
        } else {
            Err(jsonrpsee::core::Error::Custom(String::from(
                "Account session is not available",
            )))
        }
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
        let durable_app_id = ctx.app_id.to_string();
        let environment = config.options.environment;
        let distributor_app_id = self
            .state
            .get_device_manifest()
            .get_distributor_experience_id();
        let params = RoleInfo {
            capability: FireboltCap::short("advertising:identifier".to_string()),
            role: Some(CapabilityRole::Use),
        };
        let ad_id_authorised = is_permitted(&self.state, &ctx, &params)
            .await
            .unwrap_or(false);

        let ad_opt_out = PrivacyImpl::get_allow_app_content_ad_targeting(&self.state).await;

        let mut privacy_data = privacy_rpc::get_allow_app_content_ad_targeting_settings(
            &self.state,
            None,
            &durable_app_id,
            &ctx,
        )
        .await;

        privacy_data.insert("pdt".into(), "gdp:v1".into());

        let coppa = match config.options.coppa {
            Some(c) => c as u32,
            None => 0,
        };

        let advertising_request = AdvertisingRequest::GetAdConfig(AdConfigRequestParams {
            privacy_data: privacy_data.clone(),
            durable_app_id: durable_app_id.clone(),
            dist_session: session
                .clone()
                .ok_or_else(|| Error::Custom(String::from("no session available")))?,
            environment: environment.to_string(),
            scope: get_scope_option_map(&None),
        });

        debug!("config: advertising_request: {:?}", advertising_request);

        let ad_config = match self
            .state
            .get_client()
            .send_extn_request(advertising_request)
            .await
        {
            Ok(message) => match message.payload.extract() {
                Some(advertising_resp) => match advertising_resp {
                    AdvertisingResponse::AdConfig(resp) => resp,
                    _ => {
                        error!("config: Unexpected response payload, ad config not available");
                        AdConfigResponse::default()
                    }
                },
                None => {
                    error!("config: Ad config payload missing");
                    AdConfigResponse::default()
                }
            },
            Err(e) => {
                error!("config: Could not get ad config: e={}", e);
                AdConfigResponse::default()
            }
        };

        let privacy_data_enc =
            base64.encode(serde_json::to_string(&privacy_data).unwrap_or_default());

        let ad_framework_config = AdvertisingFrameworkConfig {
            ad_server_url: ad_config.ad_server_url,
            ad_server_url_template: ad_config.ad_server_url_template,
            ad_network_id: ad_config.ad_network_id,
            ad_profile_id: ad_config.ad_profile_id,
            ad_site_section_id: ad_config.ad_site_section_id,
            ad_opt_out,
            privacy_data: privacy_data_enc,
            ifa: if ad_id_authorised {
                ad_config.ifa
            } else {
                IFA_ZERO_BASE64.to_string()
            },
            ifa_value: if ad_id_authorised {
                ad_config.ifa_value
            } else {
                let ifa_val_zero = ad_config
                    .ifa_value
                    .chars()
                    .map(|x| match x {
                        '-' => x,
                        _ => '0',
                    })
                    .collect();
                ifa_val_zero
            },
            app_name: durable_app_id.clone(),
            app_bundle_id: ad_config.app_bundle_id,
            distributor_app_id,
            device_ad_attributes: String::default(),
            coppa,
            authentication_entity: config.options.authentication_entity.unwrap_or_default(),
        };

        Ok(ad_framework_config)
    }

    async fn device_attributes(&self, ctx: CallContext) -> RpcResult<Value> {
        let afc = self.config(ctx.clone(), Default::default()).await?;
        let buff = base64.decode(afc.device_ad_attributes).unwrap_or_default();
        match String::from_utf8(buff) {
            Ok(mut b_string) => {
                /*
                dealing with: https://ccp.sys.comcast.net/browse/OTTX-28561
                This is actually caused by ad-platform-service getting a 404
                from Freewheel and returning an error.
                */

                if b_string.trim().is_empty() {
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
        Ok(get_advertisting_policy(&self.state).await)
    }

    async fn advertising_on_policy_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        rpc_add_event_listener_with_decorator(
            &self.state,
            ctx,
            request,
            EVENT_ADVERTISING_POLICY_CHANGED,
            Some(Box::new(AdvertisingPolicyEventDecorator {})),
        )
        .await
    }

    async fn advertising_set_skip_restriction(
        &self,
        _ctx: CallContext,
        set_request: SetSkipRestrictionRequest,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.state,
            StorageProperty::SkipRestriction,
            String::from(set_request.value.as_string()),
            None,
        )
        .await
    }

    async fn advertising_skip_restriction(&self, _ctx: CallContext) -> RpcResult<String> {
        Ok(
            StorageManager::get_string(&self.state, StorageProperty::SkipRestriction)
                .await
                .unwrap_or_else(|_| String::from(NONE)),
        )
    }

    async fn advertising_on_skip_restriction_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener_with_decorator(
            &self.state,
            EVENT_ADVERTISING_SKIP_RESTRICTION_CHANGED.to_string(),
            ctx,
            request,
            Some(Box::new(AdvertisingSetRestrictionEventDecorator {})),
        );
        Ok(ListenerResponse {
            listening: listen,
            event: EVENT_ADVERTISING_SKIP_RESTRICTION_CHANGED.to_string(),
        })
    }
}

pub struct AdvertisingRPCProvider;
impl RippleRPCProvider<AdvertisingImpl> for AdvertisingRPCProvider {
    fn provide(state: PlatformState) -> RpcModule<AdvertisingImpl> {
        (AdvertisingImpl { state }).into_rpc()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::firebolt::handlers::advertising_rpc::AdvertisingImpl;
    use ripple_sdk::{api::gateway::rpc_gateway_api::JsonRpcApiRequest, tokio};
    use ripple_tdk::utils::test_utils::Mockable;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct CallContextContainer {
        pub ctx: Option<CallContext>,
    }

    fn merge(a: &mut Value, b: &Value) {
        match (a, b) {
            (&mut Value::Object(ref mut a), Value::Object(ref b)) => {
                for (k, v) in b {
                    merge(a.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
            (a, b) => {
                *a = b.clone();
            }
        }
    }

    fn test_request(
        method_name: String,
        call_context: Option<CallContext>,
        params: Option<Value>,
    ) -> String {
        let mut the_map = params.map_or(json!({}), |v| v);
        merge(
            &mut the_map,
            &serde_json::json!(CallContextContainer { ctx: call_context }),
        );
        let v = serde_json::to_value(JsonRpcApiRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: method_name,
            params: Some(the_map),
        })
        .unwrap();
        serde_json::to_string(&v).unwrap()
    }

    #[tokio::test]
    pub async fn test_app_bundle_id() {
        let ad_module = (AdvertisingImpl {
            state: PlatformState::mock(),
        })
        .into_rpc();

        let request = test_request(
            "advertising.appBundleId".to_string(),
            Some(CallContext::mock()),
            None,
        );

        assert!(ad_module.raw_json_request(&request).await.is_ok());
    }
}
