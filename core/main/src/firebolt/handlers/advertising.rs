use crate::{
    api::handlers::capabilities::is_authorized,
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    apps::app_events::{
        AppEventDecorationError, AppEventDecorator, AppEvents, ListenRequest, ListenerResponse,
    },
    helpers::{
        ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
        rpc_util::rpc_err,
        session_util::{dab_to_dpab, get_distributor_session_from_platform_state},
    },
    managers::{
        capability_manager::{
            CapClassifiedRequest, CapRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
        },
        storage::storage_property::EVENT_ADVERTISING_POLICY_CHANGED,
    },
    platform_state::PlatformState,
};
use dpab::core::{
    message::{DpabRequestPayload, DpabResponsePayload},
    model::advertising::{AdIdRequestParams, AdInitObjectRequestParams, AdvertisingRequest},
};
use jsonrpsee::{
    core::{async_trait, Error, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use tracing::{error, instrument};

use super::privacy::{self, PrivacyImpl};

const ADVERTISING_APP_BUNDLE_ID_SUFFIX: &'static str = "Comcast";

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetAdConfig {
    pub options: AdConfig,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Prod,
    Test,
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Environment::Prod => write!(f, "prod"),
            Environment::Test => write!(f, "test"),
        }
    }
}

impl Default for Environment {
    fn default() -> Self {
        Environment::Prod
    }
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AdConfig {
    #[serde(default)]
    pub environment: Environment,
    // COPPA stands for Children's Online Privacy Protection Act.
    pub coppa: Option<bool>,
    pub authentication_entity: Option<String>,
}

impl Default for GetAdConfig {
    fn default() -> Self {
        GetAdConfig {
            options: AdConfig {
                environment: Environment::default(),
                coppa: Some(false),
                authentication_entity: Some("".to_owned()),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvertisingFrameworkConfig {
    pub ad_server_url: String,
    pub ad_server_url_template: String,
    pub ad_network_id: String,
    pub ad_profile_id: String,
    pub ad_site_section_id: String,
    pub ad_opt_out: bool,
    pub privacy_data: String,
    pub ifa_value: String,
    pub ifa: String,
    pub app_name: String,
    pub app_bundle_id: String,
    pub distributor_app_id: String,
    pub device_ad_attributes: String,
    pub coppa: u32,
    pub authentication_entity: String,
}

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
    #[method(name = "advertising.advertisingId", aliases = ["badger.advertisingId","badger.xifa"])]
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

#[derive(Clone)]
struct AdvertisingPolicyEventDecorator {}

#[async_trait]
impl AppEventDecorator for AdvertisingPolicyEventDecorator {
    async fn decorate(
        &self,
        _ps: &PlatformState,
        _ctx: &CallContext,
        _event_name: &str,
        val_in: &Value,
    ) -> Result<Value, AppEventDecorationError> {
        Ok(serde_json::to_value(AdvertisingPolicy {
            skip_restriction: "adsUnwatched".to_owned(),
            limit_ad_tracking: val_in.as_bool().unwrap(),
        })
        .unwrap())
    }
    fn dec_clone(&self) -> Box<dyn AppEventDecorator + Send + Sync> {
        Box::new(self.clone())
    }
}

pub struct AdvertisingImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}

#[async_trait]
impl AdvertisingServer for AdvertisingImpl<RippleHelper> {
    #[instrument(skip(self))]
    async fn reset_identifier(&self, _ctx: CallContext) -> RpcResult<()> {
        let cm_c = self.helper.get_config();
        let session = get_distributor_session_from_platform_state(&self.platform_state).await?;

        let payload = DpabRequestPayload::Advertising(AdvertisingRequest::ResetAdIdentifier(
            dab_to_dpab(session).unwrap(),
        ));
        let resp = self.helper.clone().send_dpab(payload).await;
        if let Err(_) = resp {
            error!("Error resetting ad identifier {:?}", resp);
            return Err(rpc_err("Could not reset ad identifier for the device"));
        }
        match resp.unwrap() {
            DpabResponsePayload::None => Ok(()),
            _ => Err(rpc_err("Unable to reset ad identifier")),
        }
    }
    #[instrument(skip(self))]
    async fn advertising_id(&self, ctx: CallContext) -> RpcResult<AdvertisingId> {
        let cm_c = self.helper.get_config();
        let session = get_distributor_session_from_platform_state(&self.platform_state).await?;

        let payload =
            DpabRequestPayload::Advertising(AdvertisingRequest::GetAdIdObject(AdIdRequestParams {
                privacy_data: privacy::get_limit_ad_tracking_settings(&self.platform_state).await,
                app_id: ctx.clone().app_id.to_string(),
                dist_session: dab_to_dpab(session).unwrap(),
            }));
        let resp = self.helper.clone().send_dpab(payload).await;
        if let Err(_) = resp {
            error!("Error getting ad init object: {:?}", resp);
            return Err(rpc_err("Could not get ad init object from the device"));
        }
        match resp.unwrap() {
            DpabResponsePayload::AdIdObject(obj) => {
                let ad_init_object = AdvertisingId {
                    ifa: obj.ifa,
                    ifa_type: obj.ifa_type,
                    lmt: obj.lmt,
                };
                Ok(ad_init_object)
            }
            _ => Err(rpc_err(
                "Device returned an invalid type for ad init object",
            )),
        }
    }

    #[instrument(skip(self))]
    fn app_bundle_id(&self, ctx: CallContext) -> RpcResult<String> {
        Ok(format!(
            "{}.{}",
            ctx.app_id, ADVERTISING_APP_BUNDLE_ID_SUFFIX
        ))
    }

    #[instrument(skip(self))]
    async fn badger_init_object(
        &self,
        ctx: CallContext,
        config: AdConfig,
    ) -> RpcResult<AdvertisingFrameworkConfig> {
        self.config(ctx, GetAdConfig { options: config }).await
    }
    async fn config(
        &self,
        ctx: CallContext,
        config: GetAdConfig,
    ) -> RpcResult<AdvertisingFrameworkConfig> {
        let cm_c = self.helper.get_config();
        let session = get_distributor_session_from_platform_state(&self.platform_state).await?;
        let ad_id_authorised = is_authorized(&self.helper, &ctx, "advertising:identifier").await;

        let payload = DpabRequestPayload::Advertising(AdvertisingRequest::GetAdInitObject(
            AdInitObjectRequestParams {
                privacy_data: privacy::get_limit_ad_tracking_settings(&self.platform_state).await,
                environment: config.options.environment.to_string(),
                durable_app_id: ctx.app_id.to_string(),
                app_version: "".to_string(),
                distributor_app_id: self.helper.get_config().distributor_experience_id(),
                device_ad_attributes: HashMap::new(),
                coppa: config.options.coppa.unwrap_or(false),
                authentication_entity: config.options.authentication_entity.unwrap_or_default(),
                dist_session: dab_to_dpab(session).unwrap(),
            },
        ));
        let resp = self.helper.clone().send_dpab(payload).await;
        if let Err(_) = resp {
            error!("Error getting ad init object: {:?}", resp);
            return Err(rpc_err("Could not get ad init object from the device"));
        }
        match resp.unwrap() {
            DpabResponsePayload::AdInitObject(obj) => {
                let ad_init_object = AdvertisingFrameworkConfig {
                    ad_server_url: obj.ad_server_url,
                    ad_server_url_template: obj.ad_server_url_template,
                    ad_network_id: obj.ad_network_id,
                    ad_profile_id: obj.ad_profile_id,
                    ad_site_section_id: "".to_string(),
                    ad_opt_out: obj.ad_opt_out,
                    privacy_data: obj.privacy_data,
                    ifa: if (ad_id_authorised) {
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
        }
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    async fn policy(&self, _ctx: CallContext) -> RpcResult<AdvertisingPolicy> {
        let limit_ad_tracking = PrivacyImpl::get_limit_ad_tracking(&self.platform_state).await;
        Ok(AdvertisingPolicy {
            skip_restriction: "adsUnwatched".to_owned(),
            limit_ad_tracking,
        })
    }
    #[instrument(skip(self))]
    async fn advertising_on_policy_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener_with_decorator(
            &&self.platform_state.app_events_state,
            EVENT_ADVERTISING_POLICY_CHANGED.to_string(),
            ctx,
            request,
            Some(Box::new(AdvertisingPolicyEventDecorator {})),
        );
        Ok(ListenerResponse {
            listening: listen,
            event: EVENT_ADVERTISING_POLICY_CHANGED,
        })
    }
}

pub struct AdvertisingRippleProvider;

pub struct AdvertisingCapHandler;

impl IGetLoadedCaps for AdvertisingCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::Short("advertising:identifier".into()),
                FireboltCap::Short("advertising:configuration".into()),
                FireboltCap::Short("privacy:advertising".into()),
            ])]),
        }
    }
}

impl RPCProvider<AdvertisingImpl<RippleHelper>, AdvertisingCapHandler>
    for AdvertisingRippleProvider
{
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (
        RpcModule<AdvertisingImpl<RippleHelper>>,
        AdvertisingCapHandler,
    ) {
        let a = AdvertisingImpl {
            helper: rhf.clone().get(self.get_helper_variant()),
            platform_state,
        };
        (a.into_rpc(), AdvertisingCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![
            RippleHelperType::Dab,
            RippleHelperType::Dpab,
            RippleHelperType::Cap,
        ]
    }
}
