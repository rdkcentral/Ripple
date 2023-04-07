use crate::api::rpc::rpc_gateway::RPCProvider;
use crate::apps::provider_broker;
use crate::apps::provider_broker::ProviderBroker;
use crate::apps::provider_broker::ProviderResponse;
use crate::apps::provider_broker::ProviderResponsePayload;
use crate::platform_state::PlatformState;
use crate::{
    api::rpc::rpc_gateway::CallContext,
    apps::{
        app_events::{ListenRequest, ListenerResponse},
        provider_broker::FocusRequest,
    },
    helpers::ripple_helper::{RippleHelper, RippleHelperFactory, RippleHelperType},
};
use crate::{
    apps::provider_broker::ProviderRequestPayload,
    managers::capability_manager::{
        CapClassifiedRequest, FireboltCap, IGetLoadedCaps, RippleHandlerCaps,
    },
};
use jsonrpsee::{
    core::{async_trait, Error, RpcResult},
    proc_macros::rpc,
    RpcModule,
};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

pub const EMAIL_EVENT_PREFIX: &'static str = "keyboard.onRequestEmail";
pub const PASSWORD_EVENT_PREFIX: &'static str = "keyboard.onRequestPassword";
pub const STANDARD_EVENT_PREFIX: &'static str = "keyboard.onRequestStandard";

const CAPABILITY: &'static str = "xrn:firebolt:capability:input:keyboard";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum KeyboardType {
    Email,
    Password,
    Standard,
}

impl KeyboardType {
    pub fn to_provider_method(&self) -> &str {
        match self {
            KeyboardType::Email => "email",
            KeyboardType::Password => "password",
            KeyboardType::Standard => "standard",
        }
    }
}

#[derive(Deserialize)]
pub struct KeyboardRequestPassword {
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Deserialize)]
pub struct KeyboardRequestEmail {
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(rename = "type")]
    _type: EmailUsage,
}

#[derive(Deserialize)]
pub struct KeyboardRequest {
    message: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardProviderResponse {
    correlation_id: String,
    result: KeyboardResult,
}

impl KeyboardProviderResponse {
    pub fn to_provider_response(&self) -> ProviderResponse {
        ProviderResponse {
            correlation_id: self.correlation_id.clone(),
            result: ProviderResponsePayload::KeyboardResult(self.result.clone()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KeyboardSession {
    #[serde(rename = "type")]
    _type: KeyboardType,
    message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KeyboardResult {
    text: String,
    canceled: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct PromptEmailResult {
    status: PromptEmailStatus,
    data: PromptEmailData,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PromptEmailRequest {
    prefill_type: PrefillType,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum PrefillType {
    SignIn,
    SignUp,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmailUsage {
    SignIn,
    SignUp,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PromptEmailStatus {
    Success,
    Dismiss,
}

#[derive(Serialize, Debug, Clone)]
pub struct PromptEmailData {
    email: String,
}

#[rpc(server)]
pub trait Keyboard {
    #[method(name = "keyboard.onRequestStandard")]
    async fn on_request_standard(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "keyboard.onRequestEmail")]
    async fn on_request_email(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "keyboard.onRequestPassword")]
    async fn on_request_password(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse>;

    #[method(name = "keyboard.standardFocus")]
    async fn standard_focus(
        &self,
        ctx: CallContext,
        request: FocusRequest,
    ) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.standardResponse")]
    async fn standard_response(
        &self,
        ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.emailFocus")]
    async fn email_focus(&self, ctx: CallContext, request: FocusRequest) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.emailResponse")]
    async fn email_response(
        &self,
        ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.passwordFocus")]
    async fn password_focus(
        &self,
        ctx: CallContext,
        request: FocusRequest,
    ) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.passwordResponse")]
    async fn password_response(
        &self,
        ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>>;

    #[method(name = "keyboard.standard")]
    async fn standard(&self, ctx: CallContext, request: KeyboardRequest) -> RpcResult<String>;

    #[method(name = "keyboard.email")]
    async fn email(&self, ctx: CallContext, request: KeyboardRequestEmail) -> RpcResult<String>;

    #[method(name = "keyboard.password")]
    async fn password(
        &self,
        ctx: CallContext,
        request: KeyboardRequestPassword,
    ) -> RpcResult<String>;
}

pub struct KeyboardImpl<IRippleHelper> {
    pub helper: Box<IRippleHelper>,
    pub platform_state: PlatformState,
}

#[async_trait]
impl KeyboardServer for KeyboardImpl<RippleHelper> {
    async fn on_request_standard(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_session(ctx, request, KeyboardType::Standard, STANDARD_EVENT_PREFIX)
            .await
    }
    async fn on_request_email(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_session(ctx, request, KeyboardType::Email, EMAIL_EVENT_PREFIX)
            .await
    }
    async fn on_request_password(
        &self,
        ctx: CallContext,
        request: ListenRequest,
    ) -> RpcResult<ListenerResponse> {
        self.on_request_session(ctx, request, KeyboardType::Password, PASSWORD_EVENT_PREFIX)
            .await
    }

    async fn standard_response(
        &self,
        _ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>> {
        let msg = resp.to_provider_response();
        ProviderBroker::provider_response(&self.platform_state, msg).await;
        Ok(None)
    }

    async fn standard_focus(
        &self,
        ctx: CallContext,
        request: FocusRequest,
    ) -> RpcResult<Option<()>> {
        ProviderBroker::focus(&self.platform_state, ctx, CAPABILITY.to_string(), request).await;
        Ok(None)
    }

    async fn email_response(
        &self,
        _ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>> {
        let msg = resp.to_provider_response();
        ProviderBroker::provider_response(&self.platform_state, msg).await;
        Ok(None)
    }

    async fn email_focus(&self, ctx: CallContext, request: FocusRequest) -> RpcResult<Option<()>> {
        ProviderBroker::focus(&self.platform_state, ctx, CAPABILITY.to_string(), request).await;
        Ok(None)
    }

    async fn password_response(
        &self,
        _ctx: CallContext,
        resp: KeyboardProviderResponse,
    ) -> RpcResult<Option<()>> {
        let msg = resp.to_provider_response();
        ProviderBroker::provider_response(&self.platform_state, msg).await;
        Ok(None)
    }

    async fn password_focus(
        &self,
        ctx: CallContext,
        request: FocusRequest,
    ) -> RpcResult<Option<()>> {
        ProviderBroker::focus(&self.platform_state, ctx, CAPABILITY.to_string(), request).await;
        Ok(None)
    }

    async fn standard(&self, ctx: CallContext, request: KeyboardRequest) -> RpcResult<String> {
        Ok(self
            .call_keyboard_provider(ctx, request, KeyboardType::Standard)
            .await?
            .text)
    }

    async fn email(&self, ctx: CallContext, request: KeyboardRequestEmail) -> RpcResult<String> {
        let req = KeyboardRequest {
            message: request.message.unwrap_or_default(),
        };
        Ok(self
            .call_keyboard_provider(ctx, req, KeyboardType::Email)
            .await?
            .text)
    }

    async fn password(
        &self,
        ctx: CallContext,
        request: KeyboardRequestPassword,
    ) -> RpcResult<String> {
        let req = KeyboardRequest {
            message: request.message.unwrap_or_default(),
        };
        Ok(self
            .call_keyboard_provider(ctx, req, KeyboardType::Password)
            .await?
            .text)
    }

    async fn prompt_email(
        &self,
        ctx: CallContext,
        request: PromptEmailRequest,
    ) -> RpcResult<PromptEmailResult> {
        let keyboard_req = KeyboardRequest {
            message: match request.prefill_type {
                PrefillType::SignIn => "Sign in".to_string(),
                PrefillType::SignUp => "Sign up".to_string(),
            },
        };
        let result = self
            .call_keyboard_provider(ctx, keyboard_req, KeyboardType::Email)
            .await?;
        let status = match result.canceled {
            true => PromptEmailStatus::Dismiss,
            false => PromptEmailStatus::Success,
        };
        Ok(PromptEmailResult {
            status,
            data: PromptEmailData { email: result.text },
        })
    }
}

impl KeyboardImpl<RippleHelper> {
    async fn call_keyboard_provider(
        &self,
        ctx: CallContext,
        request: KeyboardRequest,
        typ: KeyboardType,
    ) -> RpcResult<KeyboardResult> {
        let method = String::from(typ.to_provider_method());
        let session = KeyboardSession {
            _type: typ,
            message: request.message,
        };
        let (session_tx, session_rx) = oneshot::channel::<ProviderResponsePayload>();
        let pr_msg = provider_broker::Request {
            // TODO which capability this rpc method providers should come from firebolt spec
            capability: CAPABILITY.to_string(),
            method: method,
            caller: ctx,
            request: ProviderRequestPayload::KeyboardSession(session),
            tx: session_tx,
            app_id: None,
        };
        ProviderBroker::invoke_method(&self.platform_state, pr_msg).await;
        match session_rx.await {
            Ok(result) => match result.as_keyboard_result() {
                Some(res) => Ok(res),
                None => Err(Error::Custom(String::from(
                    "Invalid response back from provider",
                ))),
            },
            Err(_) => Err(Error::Custom(String::from(
                "Error returning back from keyboard provider",
            ))),
        }
    }

    async fn on_request_session(
        &self,
        ctx: CallContext,
        request: ListenRequest,
        typ: KeyboardType,
        event_name: &'static str,
    ) -> RpcResult<ListenerResponse> {
        let listen = request.listen;
        let method = String::from(typ.to_provider_method());
        // TODO which capability this rpc method providers should come from firebolt spec
        ProviderBroker::register_or_unregister_provider(
            &self.platform_state,
            String::from(CAPABILITY),
            method,
            event_name,
            ctx,
            request,
        )
        .await;
        Ok(ListenerResponse {
            listening: listen,
            event: event_name,
        })
    }
}

pub struct KeyboardProvider;

pub struct KeyboardCapHandler;

impl IGetLoadedCaps for KeyboardCapHandler {
    fn get_loaded_caps(&self) -> RippleHandlerCaps {
        RippleHandlerCaps {
            caps: Some(vec![
                CapClassifiedRequest::Supported(vec![
                    FireboltCap::Short("input:keyboard".into()),
                ]),
                CapClassifiedRequest::NotAvailable(vec![FireboltCap::Short(
                    "input:keyboard".into(),
                )]),
            ]),
        }
    }
}

impl RPCProvider<KeyboardImpl<RippleHelper>, KeyboardCapHandler> for KeyboardProvider {
    fn provide(
        self,
        rhf: Box<RippleHelperFactory>,
        platform_state: PlatformState,
    ) -> (RpcModule<KeyboardImpl<RippleHelper>>, KeyboardCapHandler) {
        let a = KeyboardImpl {
            helper: rhf.get(self.get_helper_variant()),
            platform_state,
        };
        (a.into_rpc(), KeyboardCapHandler)
    }

    fn get_helper_variant(self) -> Vec<RippleHelperType> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use dab::core::{
        message::{DabError, DabRequest, DabRequestPayload, DabResponsePayload},
        model::device::DeviceRequest,
    };

    use tokio::sync::{
        mpsc::{self, channel, Receiver},
        oneshot,
    };

    use dpab::core::message::DpabResponsePayload;

    use super::{KeyboardImpl, KeyboardRequest, KeyboardResult, KeyboardServer, KeyboardType};
    use crate::{
        api::{
            handlers::keyboard::{KeyboardRequestEmail, KeyboardRequestPassword},
            rpc::{api_messages::ApiProtocol, rpc_gateway::CallContext},
        },
        apps::provider_broker::ProviderRequestPayload,
        apps::test::helpers::get_ripple_helper_factory,
        helpers::ripple_helper::mock_ripple_helper::MockRippleBuilder,
        helpers::ripple_helper::RippleHelper,
        platform_state::PlatformState,
    };
    use jsonrpsee::{
        core::{async_trait, Error, RpcResult},
        proc_macros::rpc,
        RpcModule,
    };

    #[tokio::test]
    async fn test_keyboard_email() {
        let helper: RippleHelper = RippleHelper::default();
        let mut platform_state = PlatformState::new(helper.clone());

        let under_test = KeyboardImpl {
            helper: Box::new(helper),
            platform_state: platform_state,
        };

        let ctx = CallContext {
            session_id: "some_id".into(),
            request_id: "1".into(),
            app_id: "some_app".into(),
            call_id: 1,
            protocol: ApiProtocol::JsonRpc,
            method: "keyboard.email".into(),
        };

        let request = KeyboardRequestEmail {
            message: Some("abc".to_string()),
            _type: super::EmailUsage::SignIn,
        };

        under_test.email(ctx, request);
    }

    #[tokio::test]
    async fn test_keyboard_password() {
        let helper: RippleHelper = RippleHelper::default();
        let mut platform_state = PlatformState::new(helper.clone());

        let under_test = KeyboardImpl {
            helper: Box::new(helper),
            platform_state: platform_state,
        };

        let ctx = CallContext {
            session_id: "some_id".into(),
            request_id: "1".into(),
            app_id: "some_app".into(),
            call_id: 1,
            protocol: ApiProtocol::JsonRpc,
            method: "keyboard.password".into(),
        };

        let request = KeyboardRequestPassword {
            message: Some("abc".to_string()),
        };

        under_test.password(ctx, request);
    }
}
