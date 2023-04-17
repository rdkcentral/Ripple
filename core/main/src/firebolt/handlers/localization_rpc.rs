use crate::{
    api::rpc::rpc_gateway::{CallContext, RPCProvider},
    apps::app_events::{AppEvents, ListenRequest, ListenerResponse},
    helpers::ripple_helper::{IRippleHelper, RippleHelper, RippleHelperFactory, RippleHelperType},
    managers::{
        capability_manager::{
            CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
        },
        storage::{
            storage_manager::StorageManager,
            storage_property::{StorageProperty, KEY_POSTAL_CODE},
        },
    },
    platform_state::PlatformState,
};

use crate::helpers::serde_utils::*;
use std::collections::HashMap;

use dab::core::{
    message::{DabRequestPayload, DabResponsePayload},
    model::device::DeviceRequest,
};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use serde::{Deserialize, Serialize};

const EVENT_TIMEZONE_CHANGED: &'static str = "localization.onTimeZoneChanged";

use tracing::{error, instrument};
#[derive(Deserialize, Debug)]
pub struct SetStringProperty {
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct LanguageProperty {
    #[serde(with = "language_code_serde")]
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TimezoneProperty {
    #[serde(with = "timezone_serde")]
    pub value: String,
}

#[rpc(server)]
pub trait Localization {
    #[method(name = "localization.locality")]
    async fn locality(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setLocality")]
    async fn locality_set(&self, ctx: CallContext, set_request: SetStringProperty)
        -> RpcResult<()>;
    #[method(name = "localization.onLocalityChanged")]
    async fn on_locality_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.countryCode")]
    async fn country_code(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setCountryCode")]
    async fn country_code_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "localization.onCountryCodeChanged")]
    async fn on_country_code_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.language")]
    async fn language(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setLanguage")]
    async fn language_set(&self, ctx: CallContext, set_request: LanguageProperty) -> RpcResult<()>;
    #[method(name = "localization.onLanguageChanged")]
    async fn on_language_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.postalCode")]
    async fn postal_code(&self, _ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setPostalCode")]
    async fn postal_code_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "localization.onPostalCodeChanged")]
    async fn on_postal_code_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.locale")]
    async fn locale(&self, _ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setLocale")]
    async fn locale_set(&self, ctx: CallContext, set_request: SetStringProperty) -> RpcResult<()>;
    #[method(name = "localization.onLocaleChanged")]
    async fn on_locale_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.latlon")]
    async fn latlon(&self, _ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.setLatlon")]
    async fn latlon_set(&self, ctx: CallContext, set_request: SetStringProperty) -> RpcResult<()>;
    #[method(name = "localization.onLatlonChanged")]
    async fn on_latlon_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.additionalInfo")]
    async fn additional_info(&self, _ctx: CallContext) -> RpcResult<HashMap<String, String>>;
    #[method(name = "localization.setAdditionalInfo")]
    async fn additional_info_set(
        &self,
        ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()>;
    #[method(name = "localization.onAdditionalInfoChanged")]
    async fn on_additional_info_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
    #[method(name = "localization.setTimeZone")]
    async fn timezone_set(&self, ctx: CallContext, set_request: TimezoneProperty) -> RpcResult<()>;
    #[method(name = "localization.timeZone")]
    async fn timezone(&self, ctx: CallContext) -> RpcResult<String>;
    #[method(name = "localization.onTimeZoneChanged")]
    async fn on_timezone_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;
}

#[derive(Debug)]
pub struct LocalizationImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}

impl LocalizationImpl<RippleHelper> {
    pub async fn postal_code(state: &PlatformState, app_id: String) -> Option<String> {
        match StorageManager::get_string(state, StorageProperty::PostalCode).await {
            Ok(resp) => Some(resp),
            Err(_) => {
                match StorageManager::get_string_from_namespace(state, app_id, KEY_POSTAL_CODE)
                    .await
                {
                    Ok(resp) => Some(resp.as_value()),
                    Err(_) => None,
                }
            }
        }
    }
}

#[async_trait]
impl LocalizationServer for LocalizationImpl<RippleHelper> {
    #[instrument(skip(self))]
    async fn locality(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.platform_state, StorageProperty::Locality).await
    }

    #[instrument(skip(self))]
    async fn locality_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::Locality,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn on_locality_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            "localization.onLocalityChanged".to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: "localization.onLocalityChanged",
        })
    }

    #[instrument(skip(self))]
    async fn country_code(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.platform_state, StorageProperty::CountryCode).await
    }

    #[instrument(skip(self))]
    async fn country_code_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::CountryCode,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn on_country_code_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            "localization.onCountryCodeChanged".to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: "localization.onCountryCodeChanged",
        })
    }

    #[instrument(skip(self))]
    async fn language(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.platform_state, StorageProperty::Language).await
    }

    #[instrument(skip(self))]
    async fn language_set(
        &self,
        _ctx: CallContext,
        set_request: LanguageProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::Language,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn on_language_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            "localization.onLanguageChanged".to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: "localization.onLanguageChanged",
        })
    }

    #[instrument(skip(self))]
    async fn postal_code(&self, ctx: CallContext) -> RpcResult<String> {
        match LocalizationImpl::postal_code(&self.platform_state, ctx.app_id).await {
            Some(postal_code) => Ok(postal_code),
            None => Err(StorageManager::get_firebolt_error(
                &StorageProperty::PostalCode,
            )),
        }
    }

    #[instrument(skip(self))]
    async fn postal_code_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::PostalCode,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn on_postal_code_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            "localization.onPostalCodeChanged".to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: "localization.onPostalCodeChanged",
        })
    }

    #[instrument(skip(self))]
    async fn locale(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.platform_state, StorageProperty::Locale).await
    }

    #[instrument(skip(self))]
    async fn locale_set(&self, ctx: CallContext, set_request: SetStringProperty) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::Locale,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn on_locale_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            "localization.onLocaleChanged".to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: "localization.onLocaleChanged",
        })
    }

    #[instrument(skip(self))]
    async fn latlon(&self, _ctx: CallContext) -> RpcResult<String> {
        StorageManager::get_string(&self.platform_state, StorageProperty::LatLon).await
    }

    #[instrument(skip(self))]
    async fn latlon_set(&self, _ctx: CallContext, set_request: SetStringProperty) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::LatLon,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn on_latlon_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            "localization.onLatlonChanged".to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: "localization.onLatlonChanged",
        })
    }

    #[instrument(skip(self))]
    async fn additional_info(&self, _ctx: CallContext) -> RpcResult<HashMap<String, String>> {
        let json_str =
            StorageManager::get_string(&self.platform_state, StorageProperty::AdditionalInfo).await;
        match json_str {
            Ok(s) => {
                let deserialized = serde_json::from_str::<HashMap<String, String>>(&s).unwrap();
                return Ok(deserialized);
            }
            Err(_e) => {
                return Err(jsonrpsee::core::Error::Custom(String::from(
                    "additional_info: error response TBD",
                )))
            }
        }
    }

    #[instrument(skip(self))]
    async fn additional_info_set(
        &self,
        _ctx: CallContext,
        set_request: SetStringProperty,
    ) -> RpcResult<()> {
        StorageManager::set_string(
            &self.platform_state,
            StorageProperty::AdditionalInfo,
            set_request.value,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn on_additional_info_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            "localization.onAdditionalInfoChanged".to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: "localization.onAdditionalInfoChanged",
        })
    }

    #[instrument(skip(self))]
    async fn timezone_set(
        &self,
        _ctx: CallContext,
        set_request: TimezoneProperty,
    ) -> RpcResult<()> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(
                DeviceRequest::GetAvailableTimezones,
            ))
            .await;

        if let Err(_e) = resp {
            return Err(jsonrpsee::core::Error::Custom(String::from(
                "timezone_set: error response TBD",
            )));
        }

        if let DabResponsePayload::AvailableTimezones(timezones) = resp.unwrap() {
            if !timezones.contains(&set_request.value) {
                error!(
                    "timezone_set: Unsupported timezone: tz={}",
                    set_request.value
                );
                return Err(jsonrpsee::core::Error::Custom(String::from(
                    "timezone_set: error response TBD",
                )));
            }
        } else {
            return Err(jsonrpsee::core::Error::Custom(String::from(
                "timezone_set: error response TBD",
            )));
        }

        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::SetTimezone(
                set_request.value.clone(),
            )))
            .await;
        match resp {
            Ok(_) => {
                AppEvents::emit(
                    &self.platform_state,
                    &EVENT_TIMEZONE_CHANGED.to_string(),
                    &serde_json::to_value(set_request.value).unwrap(),
                )
                .await;
                Ok(())
            }
            Err(_e) => Err(jsonrpsee::core::Error::Custom(String::from(
                "timezone_set: error response TBD",
            ))),
        }
    }

    #[instrument(skip(self))]
    async fn timezone(&self, _ctx: CallContext) -> RpcResult<String> {
        let resp = self
            .helper
            .send_dab(DabRequestPayload::Device(DeviceRequest::GetTimezone))
            .await;

        if let Err(_e) = resp {
            return Err(jsonrpsee::core::Error::Custom(String::from(
                "timezone: error response TBD",
            )));
        }

        if let DabResponsePayload::Timezone(timezone) = resp.unwrap() {
            Ok(timezone)
        } else {
            Err(jsonrpsee::core::Error::Custom(String::from(
                "timezone: error response TBD",
            )))
        }
    }

    #[instrument(skip(self))]
    async fn on_timezone_changed(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        AppEvents::add_listener(
            &&self.platform_state.app_events_state,
            EVENT_TIMEZONE_CHANGED.to_string(),
            ctx,
            request,
        );
        Ok(ListenerResponse {
            listening: listen,
            event: EVENT_TIMEZONE_CHANGED,
        })
    }
}

pub struct LocalizationProvider;
pub struct LocalizationCapHandler;

impl IGetLoadedCaps for LocalizationCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![CapClassifiedRequest::Supported(vec![
                FireboltCap::Short("localization:locality".into()),
                FireboltCap::Short("localization:country-code".into()),
                FireboltCap::Short("localization:language".into()),
                FireboltCap::Short("localization:postal-code".into()),
                FireboltCap::Short("localization:locale".into()),
                FireboltCap::Short("localization:location".into()),
                FireboltCap::Short("localization:time-zone".into()),
            ])]),
        }
    }
}

impl RPCProvider<LocalizationImpl<RippleHelper>, LocalizationCapHandler> for LocalizationProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (
        RpcModule<LocalizationImpl<RippleHelper>>,
        LocalizationCapHandler,
    ) {
        let a = LocalizationImpl {
            helper: rhf.clone().get(self.get_helper_variant()),
            platform_state,
        };
        (a.into_rpc(), LocalizationCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![RippleHelperType::Dab, RippleHelperType::Dpab]
    }
}

#[cfg(test)]
mod tests {
    use dab::core::{
        message::{DabError, DabRequest, DabRequestPayload, DabResponsePayload},
        model::device::DeviceRequest,
    };
    use tokio::sync::mpsc::channel;

    use crate::{
        api::rpc::{api_messages::ApiProtocol, rpc_gateway::CallContext},
        apps::test::helpers::get_ripple_helper_factory,
        helpers::ripple_helper::RippleHelper,
        platform_state::PlatformState,
    };

    use super::{LocalizationImpl, LocalizationServer, TimezoneProperty};

    #[tokio::test]
    async fn test_timezone_set_dab_failure() {
        let platform_state = PlatformState::default();
        let mut helper = RippleHelper::default();
        let mut helper_factory = get_ripple_helper_factory();
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        helper_factory.dab_tx = dab_tx.clone();
        helper.sender_hub.dab_tx = Some(dab_tx.clone());

        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    DabRequestPayload::Device(device_request) => match &device_request {
                        DeviceRequest::GetAvailableTimezones => {
                            req.respond_and_log(Err(DabError::OsError))
                        }
                        _ => panic!(),
                    },
                    _ => (),
                }
            }
        });

        let localization_server = LocalizationImpl {
            helper: Box::new(helper),
            platform_state,
        };

        let ctx = CallContext {
            session_id: "some_id".into(),
            request_id: "1".into(),
            app_id: "some_app".into(),
            call_id: 1,
            protocol: ApiProtocol::JsonRpc,
            method: "localization.setTimeZone".into(),
        };

        let request = TimezoneProperty {
            value: "America/New_York".into(),
        };

        let resp = localization_server.timezone_set(ctx, request).await;

        assert!(matches!(resp, Err(_)));
    }

    #[tokio::test]
    async fn test_timezone_set_success() {
        let platform_state = PlatformState::default();
        let mut helper = RippleHelper::default();
        let mut helper_factory = get_ripple_helper_factory();
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        helper_factory.dab_tx = dab_tx.clone();
        helper.sender_hub.dab_tx = Some(dab_tx.clone());

        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    DabRequestPayload::Device(device_request) => match &device_request {
                        DeviceRequest::GetAvailableTimezones => {
                            req.respond_and_log(Ok(DabResponsePayload::AvailableTimezones(vec![
                                "America/New_York".into(),
                            ])))
                        }
                        DeviceRequest::SetTimezone(_timezone) => {
                            req.respond_and_log(Ok(DabResponsePayload::None))
                        }
                        _ => panic!(),
                    },
                    _ => (),
                }
            }
        });

        let localization_server = LocalizationImpl {
            helper: Box::new(helper),
            platform_state,
        };

        let ctx = CallContext {
            session_id: "some_id".into(),
            request_id: "1".into(),
            app_id: "some_app".into(),
            call_id: 1,
            protocol: ApiProtocol::JsonRpc,
            method: "localization.setTimeZone".into(),
        };

        let request = TimezoneProperty {
            value: "America/New_York".into(),
        };

        let resp = localization_server.timezone_set(ctx, request).await;
        assert!(matches!(resp, Ok(_)));
    }

    #[tokio::test]
    async fn test_timezone_set_unsupported_timezone() {
        let platform_state = PlatformState::default();
        let mut helper = RippleHelper::default();
        let mut helper_factory = get_ripple_helper_factory();
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        helper_factory.dab_tx = dab_tx.clone();
        helper.sender_hub.dab_tx = Some(dab_tx.clone());

        tokio::spawn(async move {
            while let Some(req) = dab_rx.recv().await {
                match &req.payload {
                    DabRequestPayload::Device(device_request) => match &device_request {
                        DeviceRequest::GetAvailableTimezones => {
                            req.respond_and_log(Ok(DabResponsePayload::AvailableTimezones(vec![
                                "America/New_York".into(),
                            ])))
                        }
                        _ => panic!(),
                    },
                    _ => (),
                }
            }
        });

        let localization_server = LocalizationImpl {
            helper: Box::new(helper),
            platform_state,
        };

        let ctx = CallContext {
            session_id: "some_id".into(),
            request_id: "1".into(),
            app_id: "some_app".into(),
            call_id: 1,
            protocol: ApiProtocol::JsonRpc,
            method: "localization.setTimeZone".into(),
        };

        let request = TimezoneProperty {
            value: "Asia/Tokyo".into(),
        };

        let resp = localization_server.timezone_set(ctx, request).await;
        assert!(matches!(resp, Err(_)));
    }

    #[tokio::test]
    async fn test_localization_additional_info() {
        let platform_state = PlatformState::default();
        let mut helper = RippleHelper::default();
        let mut helper_factory = get_ripple_helper_factory();
        let (dab_tx, mut dab_rx) = channel::<DabRequest>(1);
        helper_factory.dab_tx = dab_tx.clone();
        helper.sender_hub.dab_tx = Some(dab_tx.clone());

        let localization_server = LocalizationImpl {
            helper: Box::new(helper),
            platform_state,
        };

        let ctx = CallContext {
            session_id: "some_id".into(),
            request_id: "1".into(),
            app_id: "some_app".into(),
            call_id: 1,
            protocol: ApiProtocol::JsonRpc,
            method: "localization.additionalInfo".into(),
        };

        let resp = localization_server.additional_info(ctx).await;
        assert!(matches!(resp, Ok(_)));
    }
}
