use crate::{
    api::{
        permissions::user_grants::{GrantActiveState, UserGrants},
        rpc::rpc_gateway::CallContext,
    },
    managers::{
        capability_manager::{
            deny_perm, DenyReason, DenyReasonWithCap, FireboltCap, FireboltPermission,
        },
        config_manager::ConfigManager,
    },
    platform_state::PlatformState,
};

use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct GrantCapManager {
    pub ps: PlatformState,
    pub caps_needing_grants: Vec<String>,
}

impl GrantCapManager {
    pub fn get(cm: Box<ConfigManager>, ps: PlatformState) -> Self {
        GrantCapManager {
            ps,
            caps_needing_grants: cm.get_caps_needing_grant(),
        }
    }

    pub async fn check_with_roles(
        &self,
        call_ctx: &CallContext,
        r: Vec<FireboltPermission>,
    ) -> Result<(), DenyReasonWithCap> {
        /*
         * Instead of just checking for grants perviously, if the user grants are not present,
         * we are taking necessary steps to get the user grant and send back the result.
         */

        for permission in r {
            let result = UserGrants::get_stored_grants_for_permission(
                &self.ps.clone(),
                call_ctx,
                &permission,
            )
            .await;
            match result {
                GrantActiveState::ActiveGrant(grant) => {
                    if grant.is_err() {
                        return deny_perm(grant.err().unwrap(), permission);
                    }
                }
                GrantActiveState::PendingGrant => {
                    let result = UserGrants::determine_grant_policies_for_permission(
                        &self.ps.clone(),
                        call_ctx,
                        &permission,
                    )
                    .await;

                    if result.is_err() {
                        return deny_perm(result.err().unwrap(), permission);
                    }
                }
            }
        }

        Ok(())

        // UserGrants::determine_grant_policies(&self.ps.clone(), call_ctx, &r).await
    }

    pub fn check(self, app_id: String, capability: FireboltCap) -> (Result<(), DenyReason>, Self) {
        if !self
            .caps_needing_grants
            .contains(&capability.clone().as_str())
        {
            return (Ok(()), self);
        }
        (
            UserGrants::check(&self.ps.clone(), app_id, capability),
            self,
        )
    }

    pub fn check_all(
        self,
        app_id: String,
        caps: Vec<String>,
    ) -> (HashMap<String, Result<(), DenyReason>>, Self) {
        let caps_hash: HashSet<String> = caps.into_iter().collect();
        let filtered_caps: HashSet<String> = caps_hash
            .clone()
            .into_iter()
            .filter(|x| !self.caps_needing_grants.contains(&x))
            .collect();
        let delta_caps = &caps_hash - &filtered_caps;
        let mut map = HashMap::new();
        filtered_caps.into_iter().for_each(|x| {
            map.insert(x.clone(), Ok(()));
        });
        let delta_list: Vec<String> = delta_caps.into_iter().collect();
        map.extend(UserGrants::check_all(&self.ps.clone(), app_id, delta_list));
        (map, self)
    }
}
#[cfg(test)]
pub mod tests {
    use dpab::core::message::DpabRequest;
    use futures::Future;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use tokio::sync::mpsc;
    use tracing::debug;

    use crate::api::handlers::pin_challenge::{
        PinChallengeImpl, PinChallengeRequest, PinChallengeResponse, PinChallengeResultReason,
        PinChallengeServer,
    };
    use crate::api::permissions::user_grants::{ChallengeRequestor, ChallengeResponse, UserGrants};
    use crate::api::permissions::user_grants_data::AutoApplyPolicy;
    use crate::apps::app_mgr::AppManagerResponse;
    use crate::helpers::channel_util::oneshot_send_and_log;
    use crate::managers::capability_manager::{
        deny_cap, CapabilityRole, DenyReason, DenyReasonWithCap, FireboltPermission,
    };
    use crate::managers::capability_resolver::{FireboltOpenRpcMethod, FireboltOpenRpcTag};
    use crate::managers::storage::storage_manager::StorageManager;
    use crate::managers::storage::storage_property::StorageProperty;
    use crate::{
        api::permissions::{
            user_grants::Challenge,
            user_grants_data::{
                GrantPolicies, GrantPolicy, GrantRequirements, GrantStep, Lifespan, PrivacySetting,
                Scope,
            },
        },
        api::{
            handlers::acknowledge_challenge::{
                AcknowledgeChallengeImpl, AcknowledgeChallengeServer,
            },
            rpc::{
                api_messages::{ApiMessage, ApiProtocol},
                firebolt_gateway::tests::TestGateway,
                rpc_gateway::CallContext,
            },
        },
        apps::{
            app_events::ListenRequest,
            app_mgr::AppRequest,
            provider_broker::{ExternalProviderRequest, ExternalProviderResponse},
        },
        helpers::ripple_helper::RippleHelper,
        managers::{capability_manager::CapRequest, config_manager::ConfigManager},
        platform_state::PlatformState,
    };
    use dab::core::message::{DabError, DabRequest, DabResponsePayload};
    #[derive(Debug, Clone)]
    pub enum AppBehaviour {
        AllGranted,
        AllDenied,
        FirstDenied,
        LastDenied,
        FirstUnSupported,
        FirstUnavailable,
    }
    impl ConfigManager {
        pub fn create_grant_policy(
            no_of_requirements: u8,
            max_no_of_steps: u8,
            app_behaviour: AppBehaviour,
            scope: Scope,
            lifespan: Lifespan,
            overrideable: bool,
            lifespan_ttl: Option<u64>,
            privacy_settings: Option<PrivacySetting>,
        ) -> GrantPolicy {
            fn get_grant_step(index: u8) -> GrantStep {
                let ack_request = GrantStep {
                    capability: "xrn:firebolt:capability:usergrant:acknowledgechallenge".to_owned(),
                    configuration: None,
                };
                let pin_request = GrantStep {
                    capability: "xrn:firebolt:capability:usergrant:pinchallenge".to_owned(),
                    configuration: Some(json!({"pinSpace" : "purchase"})),
                };
                if index % 2 == 0 {
                    ack_request
                } else {
                    pin_request
                }
            }
            let mut grant_policy: GrantPolicy = Default::default();
            grant_policy.scope = scope;
            grant_policy.lifespan = lifespan;
            grant_policy.lifespan_ttl = lifespan_ttl;
            grant_policy.overridable = overrideable;
            grant_policy.privacy_setting = privacy_settings;
            match app_behaviour {
                AppBehaviour::AllGranted => {
                    for _count in 0..no_of_requirements {
                        let mut grant_req: GrantRequirements = Default::default();
                        for _steps in 0..max_no_of_steps {
                            grant_req.steps.push(get_grant_step(_steps));
                        }
                        grant_policy.options.push(grant_req);
                    }
                }
                AppBehaviour::AllDenied => {
                    for _count in 0..no_of_requirements {
                        let mut grant_req: GrantRequirements = Default::default();
                        for _steps in 0..max_no_of_steps {
                            grant_req.steps.push(get_grant_step(_steps));
                        }
                        grant_policy.options.push(grant_req);
                    }
                }
                AppBehaviour::FirstDenied => {
                    for _count in 0..no_of_requirements {
                        let mut grant_req: GrantRequirements = Default::default();
                        grant_req.steps.push(GrantStep {
                            capability: "xrn:firebolt:capability:usergrant:acknowledgechallenge"
                                .to_owned(),
                            configuration: None,
                        });
                        for _steps in 1..max_no_of_steps {
                            grant_req.steps.push(get_grant_step(_steps));
                        }
                        grant_policy.options.push(grant_req);
                    }
                }
                AppBehaviour::LastDenied => {
                    for _count in 0..no_of_requirements {
                        let mut grant_req: GrantRequirements = Default::default();
                        for _steps in 1..max_no_of_steps {
                            grant_req.steps.push(get_grant_step(_steps));
                        }
                        grant_req.steps.push(GrantStep {
                            capability: "xrn:firebolt:capability:usergrant:acknowledgechallenge"
                                .to_owned(),
                            configuration: None,
                        });
                        grant_policy.options.push(grant_req);
                    }
                }
                AppBehaviour::FirstUnSupported => {
                    let mut grant_req: GrantRequirements = Default::default();
                    grant_req.steps.push(GrantStep {
                        capability: "xrn:firebolt:capability:usergrant:unsupported".to_owned(),
                        configuration: None,
                    });
                    for _steps in 1..max_no_of_steps {
                        grant_req.steps.push(GrantStep {
                            capability: "xrn:firebolt:capability:usergrant:acknowledgechallenge"
                                .to_owned(),
                            configuration: None,
                        });
                    }
                    grant_policy.options.push(grant_req);
                    for _count in 1..no_of_requirements {
                        let mut grant_req: GrantRequirements = Default::default();
                        for _steps in 0..max_no_of_steps {
                            grant_req.steps.push(get_grant_step(_steps));
                        }
                        if grant_req.steps.len() > 0 {
                            grant_policy.options.push(grant_req);
                        }
                    }
                }
                AppBehaviour::FirstUnavailable => {
                    let mut grant_req: GrantRequirements = Default::default();
                    grant_req.steps.push(GrantStep {
                        capability: "xrn:firebolt:capability:usergrant:unavailable".to_owned(),
                        configuration: None,
                    });
                    for _steps in 1..max_no_of_steps {
                        grant_req.steps.push(GrantStep {
                            capability: "xrn:firebolt:capability:usergrant:acknowledgechallenge"
                                .to_owned(),
                            configuration: None,
                        });
                    }
                    grant_policy.options.push(grant_req);
                    for _count in 1..no_of_requirements {
                        let mut grant_req: GrantRequirements = Default::default();
                        for _steps in 0..max_no_of_steps {
                            grant_req.steps.push(get_grant_step(_steps));
                        }
                        if grant_req.steps.len() > 0 {
                            grant_policy.options.push(grant_req);
                        }
                    }
                }
            }
            grant_policy
        }
        pub fn create_grant_policies(template_id: &str) -> GrantPolicies {
            match template_id {
                "template01" => {
                    let mut grant_policies: GrantPolicies = Default::default();
                    grant_policies._use = Some(Self::create_grant_policy(
                        1,
                        1,
                        AppBehaviour::AllGranted,
                        Scope::Device,
                        Lifespan::Once,
                        false,
                        None,
                        None,
                    ));
                    grant_policies
                }
                "template02" => {
                    let mut grant_policies: GrantPolicies = Default::default();
                    grant_policies._use = Some(Self::create_grant_policy(
                        1,
                        2,
                        AppBehaviour::AllGranted,
                        Scope::Device,
                        Lifespan::Once,
                        false,
                        None,
                        None,
                    ));
                    grant_policies
                }
                "template03" => {
                    let mut grant_policies: GrantPolicies = Default::default();
                    grant_policies._use = Some(Self::create_grant_policy(
                        1,
                        2,
                        AppBehaviour::FirstDenied,
                        Scope::Device,
                        Lifespan::Once,
                        false,
                        None,
                        None,
                    ));
                    grant_policies
                }
                "template04" => {
                    let mut grant_policies: GrantPolicies = Default::default();
                    grant_policies._use = Some(Self::create_grant_policy(
                        1,
                        2,
                        AppBehaviour::LastDenied,
                        Scope::Device,
                        Lifespan::Once,
                        false,
                        None,
                        None,
                    ));
                    grant_policies
                }
                "template05" => {
                    let mut grant_policies: GrantPolicies = Default::default();
                    grant_policies._use = Some(Self::create_grant_policy(
                        2,
                        2,
                        AppBehaviour::FirstUnSupported,
                        Scope::Device,
                        Lifespan::Once,
                        false,
                        None,
                        None,
                    ));
                    grant_policies
                }
                "template06" => {
                    let mut grant_policies: GrantPolicies = Default::default();
                    grant_policies._use = Some(Self::create_grant_policy(
                        2,
                        2,
                        AppBehaviour::FirstUnavailable,
                        Scope::Device,
                        Lifespan::Once,
                        false,
                        None,
                        None,
                    ));
                    grant_policies
                }
                "template07" => {
                    let privacy_setting = PrivacySetting {
                        property: "privacy.allowUnentitledResumePoints".to_owned(),
                        auto_apply_policy: AutoApplyPolicy::Always,
                        update_property: false,
                    };
                    let mut grant_policies: GrantPolicies = Default::default();
                    grant_policies._use = Some(Self::create_grant_policy(
                        1,
                        1,
                        AppBehaviour::AllGranted,
                        Scope::Device,
                        Lifespan::Once,
                        false,
                        None,
                        Some(privacy_setting),
                    ));
                    grant_policies
                }
                "template08" => {
                    let privacy_setting = PrivacySetting {
                        property: "privacy.allowUnentitledResumePoints".to_owned(),
                        auto_apply_policy: AutoApplyPolicy::Allowed,
                        update_property: false,
                    };
                    let mut grant_policies: GrantPolicies = Default::default();
                    grant_policies._use = Some(Self::create_grant_policy(
                        1,
                        1,
                        AppBehaviour::AllDenied,
                        Scope::Device,
                        Lifespan::Once,
                        false,
                        None,
                        Some(privacy_setting),
                    ));
                    grant_policies
                }
                "template09" => {
                    let privacy_setting = PrivacySetting {
                        property: "privacy.allowUnentitledResumePoints".to_owned(),
                        auto_apply_policy: AutoApplyPolicy::Disallowed,
                        update_property: false,
                    };
                    let mut grant_policies: GrantPolicies = Default::default();
                    grant_policies._use = Some(Self::create_grant_policy(
                        1,
                        1,
                        AppBehaviour::AllGranted,
                        Scope::Device,
                        Lifespan::Once,
                        false,
                        None,
                        Some(privacy_setting),
                    ));
                    grant_policies
                }
                "template10" => {
                    let privacy_setting = PrivacySetting {
                        property: "privacy.allowUnentitledResumePoints".to_owned(),
                        auto_apply_policy: AutoApplyPolicy::Never,
                        update_property: false,
                    };
                    let mut grant_policies: GrantPolicies = Default::default();
                    grant_policies._use = Some(Self::create_grant_policy(
                        1,
                        1,
                        AppBehaviour::AllGranted,
                        Scope::Device,
                        Lifespan::Once,
                        false,
                        None,
                        Some(privacy_setting),
                    ));
                    grant_policies
                }
                "template11" => {
                    let privacy_setting = PrivacySetting {
                        property: "privacy.allowUnentitledResumePoints".to_owned(),
                        auto_apply_policy: AutoApplyPolicy::Never,
                        update_property: false,
                    };
                    let mut grant_policies: GrantPolicies = Default::default();
                    grant_policies._use = Some(Self::create_grant_policy(
                        1,
                        1,
                        AppBehaviour::AllDenied,
                        Scope::Device,
                        Lifespan::Once,
                        false,
                        None,
                        Some(privacy_setting),
                    ));
                    grant_policies
                }
                _ => Default::default(),
            }
        }
        pub fn get_grant_policies(self: Box<Self>) -> HashMap<String, GrantPolicies> {
            let mut map = HashMap::new();

            map.insert(
                "xrn:firebolt:capability:localization:postal-code".to_owned(),
                Self::create_grant_policies("template01"),
            );
            map.insert(
                "xrn:firebolt:capability:localization:geo-location".to_owned(),
                Self::create_grant_policies("template02"),
            );
            map.insert(
                "xrn:firebolt:capability:localization:country-code".to_owned(),
                Self::create_grant_policies("template03"),
            );
            map.insert(
                "xrn:firebolt:capability:localization:language".to_owned(),
                Self::create_grant_policies("template04"),
            );
            map.insert(
                "xrn:firebolt:capability:localization:timezone".to_owned(),
                Self::create_grant_policies("template05"),
            );
            map.insert(
                "xrn:firebolt:capability:privacy:content".to_owned(),
                Self::create_grant_policies("template06"),
            );
            map.insert(
                "xrn:firebolt:capability:privacy:test1".to_owned(),
                Self::create_grant_policies("template07"),
            );
            map.insert(
                "xrn:firebolt:capability:privacy:test2".to_owned(),
                Self::create_grant_policies("template08"),
            );
            map.insert(
                "xrn:firebolt:capability:privacy:test3".to_owned(),
                Self::create_grant_policies("template09"),
            );
            map.insert(
                "xrn:firebolt:capability:privacy:test4".to_owned(),
                Self::create_grant_policies("template10"),
            );
            map.insert(
                "xrn:firebolt:capability:privacy:test5".to_owned(),
                Self::create_grant_policies("template11"),
            );

            map
        }
    }

    fn helper() -> (
        Box<RippleHelper>,
        CallContext,
        mpsc::Receiver<DabRequest>,
        mpsc::Receiver<DpabRequest>,
        mpsc::Receiver<CapRequest>,
        mpsc::Receiver<AppRequest>,
        PlatformState,
    ) {
        let (dab_tx, dab_rx) = mpsc::channel::<DabRequest>(32);
        let (dpab_tx, dpab_rx) = mpsc::channel::<DpabRequest>(32);
        let (cap_tx, cap_rx) = mpsc::channel::<CapRequest>(32);
        let (app_mgr_req_tx, app_mgr_req_rx) = mpsc::channel::<AppRequest>(32);
        let mut helper = RippleHelper::default();
        helper.sender_hub.dab_tx = Some(dab_tx.clone());
        helper.sender_hub.dpab_tx = Some(dpab_tx.clone());
        helper.sender_hub.cap_tx = Some(cap_tx.clone());
        helper.sender_hub.app_mgr_req_tx = Some(app_mgr_req_tx);
        let ctx = CallContext {
            session_id: "a".to_string(),
            request_id: "b".to_string(),
            app_id: "test".to_string(),
            call_id: 5,
            protocol: ApiProtocol::JsonRpc,
            method: "method".to_string(),
        };
        let mut ps = PlatformState::default();
        ps.services = helper.clone();
        ps.app_auth_sessions.rh = Some(Box::new(helper.clone()));
        let firebolt_openrpc_method = FireboltOpenRpcMethod {
            name: "privacy.allowUnentitledResumePoints".to_owned(),
            tags: Some(vec![FireboltOpenRpcTag {
                name: "property".to_owned(),
                allow_value: Some(true),
                uses: None,
                manages: None,
                provides: None,
                alternative: None,
                since: None,
            }]),
        };
        ps.firebolt_open_rpc.methods = vec![firebolt_openrpc_method];
        return (
            Box::new(helper),
            ctx,
            dab_rx,
            dpab_rx,
            cap_rx,
            app_mgr_req_rx,
            ps,
        );
    }

    async fn mock_channel(
        mut cap_rx: mpsc::Receiver<CapRequest>,
        mut dab_rx: mpsc::Receiver<DabRequest>,
        mut app_rx: mpsc::Receiver<AppRequest>,
    ) {
        let mut local_storage: HashMap<String, Value> = HashMap::new();
        loop {
            tokio::select! {
                data = cap_rx.recv() => {
                    debug!("Received some data in cap channel");
                    if let Some(request) = data{
                        debug!("Received cap request: {:?}", request);
                        match request {

                            CapRequest::UpdateAvailability(_,occ) => {
                                if let Some(tx) = occ{
                                    tx.callback.send(Ok(())).unwrap();
                                }
                            },
                            CapRequest::IsSupported(cap, cap_callback) => {
                                let mut result = Ok(());
                                if cap.as_str() == "xrn:firebolt:capability:usergrant:unsupported"{
                                    result = deny_cap(DenyReason::Unsupported, cap.as_str());
                                }
                                let res = cap_callback.callback.send(result);
                                debug!("Send response :{:?}", res);
                            },
                            CapRequest::IsAvailable(cap, cap_callback) => {
                                let mut result = Ok(());
                                if cap.as_str() == "xrn:firebolt:capability:usergrant:unavailable"{
                                    result = deny_cap(DenyReason::Unavailable, cap.as_str());
                                }
                                let res = cap_callback.callback.send(result);
                                debug!("Send response :{:?}", res);

                            }
                            _ => {
                                panic!("unhandled request received");
                            }
                        }
                    }
                },
                data = app_rx.recv() => {
                    if let Some(req) = data {
                        match req.method{

                            crate::apps::app_mgr::AppMethod::GetAppName(_) => {
                                let result = Ok(AppManagerResponse::AppName(Some("testapp".to_owned())));
                                oneshot_send_and_log(req.resp_tx.unwrap(), result, "AppManager response");
                            }
                            _ => {panic!("Not supposed to receive these request");}
                        }
                    }
                },
                data = dab_rx.recv() => {
                    if let Some(req) = data {
                        match &req.payload {
                            dab::core::message::DabRequestPayload::Storage(sreq) => match &sreq {
                                dab::core::model::persistent_store::StorageRequest::Get(data) => {
                                    let key = (data.namespace.to_owned() + data.key.as_str()).to_owned();
                                    if let Some(value) = local_storage.get(key.as_str()) {
                                        req.respond_and_log(Ok(DabResponsePayload::JsonValue(
                                            value.clone(),
                                        )));
                                    } else {
                                        req.respond_and_log(Err(DabError::OsError));
                                    }
                                }

                                dab::core::model::persistent_store::StorageRequest::Set(data) => {
                                    local_storage.insert(
                                        (data.namespace.to_owned() + &data.key).to_owned(),
                                        data.data.value.clone(),
                                    );
                                    req.respond_and_log(Ok(DabResponsePayload::JsonValue(json!(true))));
                                }
                            },
                            _ => panic!("Not intended to handle request other than Storage!!"),
                        }
                    }

                }
            }
        }
    }

    struct SampleApp {
        helper: Box<RippleHelper>,
    }
    impl SampleApp {
        fn new(helper: Box<RippleHelper>) -> Self {
            SampleApp { helper }
        }
        async fn start(
            &self,
            mut session_rx: mpsc::Receiver<ApiMessage>,
            platform_state: PlatformState,
            requirements_template: AppBehaviour,
        ) {
            let provider_handler = AcknowledgeChallengeImpl {
                helper: self.helper.clone(),
                platform_state: platform_state.clone(),
            };

            let pin_provider_handler = PinChallengeImpl {
                helper: self.helper.clone(),
                platform_state,
            };

            let ctx_provider = TestGateway::provider_call();
            debug!("Registering for challenge request");
            println!("Registering for challenge request");
            let _ = provider_handler
                .on_request_challenge(ctx_provider.clone(), ListenRequest { listen: true })
                .await;
            let _ = pin_provider_handler
                .on_request_challenge(ctx_provider.clone(), ListenRequest { listen: true })
                .await;
            debug!("Starting a task to listen for challenge request");
            println!("Starting a task to listen for challenge request");

            tokio::task::spawn(async move {
                debug!("Starting a loop to receive session message");
                println!("Starting a loop to receive session message");
                let mut req_count = 0;
                while let Some(message) = session_rx.recv().await {
                    let jsonrpc: Value =
                        serde_json::from_str(message.jsonrpc_msg.as_str()).unwrap();
                    debug!(
                        "received message in jsonrpc : {:?}",
                        jsonrpc.get("result").unwrap().clone()
                    );
                    println!(
                        "received message in jsonrpc : {:?}",
                        message.jsonrpc_msg.as_str()
                    );
                    if message.jsonrpc_msg.find("pinSpace").is_some() {
                        let req: ExternalProviderRequest<PinChallengeRequest> =
                            serde_json::from_value(jsonrpc.get("result").unwrap().clone()).unwrap();
                        let challenge_response = match requirements_template {
                            AppBehaviour::AllGranted => PinChallengeResponse::new(
                                true,
                                PinChallengeResultReason::CorrectPin,
                            ),
                            AppBehaviour::AllDenied
                            | AppBehaviour::FirstUnSupported
                            | AppBehaviour::FirstUnavailable => PinChallengeResponse::new(
                                false,
                                PinChallengeResultReason::ExceededPinFailures,
                            ),
                            AppBehaviour::FirstDenied => {
                                req_count += 1;
                                if req_count == 1 {
                                    PinChallengeResponse::new(
                                        false,
                                        PinChallengeResultReason::ExceededPinFailures,
                                    )
                                } else {
                                    PinChallengeResponse::new(
                                        true,
                                        PinChallengeResultReason::CorrectPin,
                                    )
                                }
                            }
                            AppBehaviour::LastDenied => {
                                req_count += 1;
                                if req_count == 1 {
                                    PinChallengeResponse::new(
                                        true,
                                        PinChallengeResultReason::CorrectPin,
                                    )
                                } else {
                                    PinChallengeResponse::new(
                                        false,
                                        PinChallengeResultReason::ExceededPinFailures,
                                    )
                                }
                            }
                        };
                        let _resp = pin_provider_handler
                            .challenge_response(
                                ctx_provider.clone(),
                                ExternalProviderResponse {
                                    correlation_id: req.correlation_id,
                                    result: challenge_response,
                                },
                            )
                            .await;
                    } else {
                        let req: ExternalProviderRequest<Challenge> =
                            serde_json::from_value(jsonrpc.get("result").unwrap().clone()).unwrap();
                        let challenge_response = match requirements_template {
                            AppBehaviour::AllGranted => ChallengeResponse { granted: true },
                            AppBehaviour::AllDenied
                            | AppBehaviour::FirstUnSupported
                            | AppBehaviour::FirstUnavailable => {
                                ChallengeResponse { granted: false }
                            }
                            AppBehaviour::FirstDenied => {
                                req_count += 1;
                                if req_count == 1 {
                                    ChallengeResponse { granted: false }
                                } else {
                                    ChallengeResponse { granted: true }
                                }
                            }
                            AppBehaviour::LastDenied => {
                                req_count += 1;
                                if req_count == 1 {
                                    ChallengeResponse { granted: true }
                                } else {
                                    ChallengeResponse { granted: false }
                                }
                            }
                        };

                        let _resp = provider_handler
                            .challenge_response(
                                ctx_provider.clone(),
                                ExternalProviderResponse {
                                    correlation_id: req.correlation_id,
                                    result: challenge_response,
                                },
                            )
                            .await;
                    }
                    // debug!("sending resp: {:?}", resp);
                }
            });
        }
    }

    pub async fn test_for_req_cap<F, T>(
        pre_setup: F,
        cap: &Vec<FireboltPermission>,
        app_behaviour: AppBehaviour,
    ) -> Result<(), DenyReasonWithCap>
    where
        F: Fn(Box<RippleHelper>, PlatformState) -> T + Send + 'static,
        T: Future<Output = ()> + Send + 'static,
    {
        let cm = ConfigManager::get();
        let (helper, ctx, dab_rx, _, cap_rx, app_events_rx, ps) = helper();
        let test_gateway = TestGateway::start(ps.clone()).await;
        // let request = ListenRequest { listen: true };
        let sample_app = SampleApp::new(helper.clone());
        tokio::spawn(async move {
            mock_channel(cap_rx, dab_rx, app_events_rx).await;
        });
        sample_app
            .start(test_gateway.prov_session_rx, ps.clone(), app_behaviour)
            .await;
        pre_setup(helper, ps.clone()).await;
        let gcm = super::GrantCapManager::get(cm.clone(), ps.clone());
        gcm.check_with_roles(&ctx, cap.clone()).await
    }
    async fn no_pre_setup(_helper: Box<RippleHelper>, _ps: PlatformState) {}

    #[tokio::test]
    pub async fn test_one_requirement_with_one_step_all_granted() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:localization:postal-code".into(),
            CapabilityRole::Use,
        )];
        let grant = test_for_req_cap(no_pre_setup, &cap, AppBehaviour::AllGranted).await;
        debug!("user grant: {:?}", grant);
        assert_eq!(grant, Ok(()));
    }
    #[tokio::test]
    async fn test_one_requirement_with_two_step_all_granted() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:localization:geo-location".into(),
            CapabilityRole::Use,
        )];
        let grant = test_for_req_cap(no_pre_setup, &cap, AppBehaviour::AllGranted).await;
        assert_eq!(grant, Ok(()));
    }

    #[tokio::test]
    async fn test_one_requirement_two_steps_first_denied() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:localization:country-code".into(),
            CapabilityRole::Use,
        )];
        let grant = test_for_req_cap(no_pre_setup, &cap, AppBehaviour::FirstDenied).await;
        assert_eq!(grant.err().unwrap().0, DenyReason::GrantDenied);
    }

    #[tokio::test]
    async fn test_one_requirement_two_steps_last_denied() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:localization:language".into(),
            CapabilityRole::Use,
        )];
        let grant = test_for_req_cap(no_pre_setup, &cap, AppBehaviour::LastDenied).await;
        assert_eq!(grant.err().unwrap().0, DenyReason::GrantDenied);
    }
    #[tokio::test]
    async fn test_first_grant_req_not_supported_others_granted() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:localization:timezone".into(),
            CapabilityRole::Use,
        )];
        let grant = test_for_req_cap(no_pre_setup, &cap, AppBehaviour::AllGranted).await;
        assert_eq!(grant, Ok(()));
    }
    #[tokio::test]
    async fn test_first_grant_req_not_available_others_granted() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:privacy:content".into(),
            CapabilityRole::Use,
        )];
        let grant = test_for_req_cap(no_pre_setup, &cap, AppBehaviour::AllGranted).await;
        assert_eq!(grant, Ok(()));
    }
    /*
     * In this test we are testing the cap xrn:firebolt:capability:privacy:test1
     * In firebolt methods we have configured x-allow-value to be true for privacy.autoUnentitledResumePoints
     * In grant policy is configured to depend on property privacy.autoUnentitledResumePoints
     * and auto update policy as always.
     *
     * During startup in the storage property, we are setting the value as true.
     *
     * As the auto apply is always, if the storage is same as that of x-allow-value, it should
     * silently grant else silently deny.
     *
     * Here storage has the same value as that of x-allow-value so cap should be granted.
     */
    #[tokio::test]
    async fn test_privacy_settings_in_policy_settings_always_allow_granted() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:privacy:test1".into(),
            CapabilityRole::Use,
        )];
        async fn pre_setup(_helper: Box<RippleHelper>, platform_state: PlatformState) {
            debug!("Pre setup called while testing");
            let _ = StorageManager::set_bool(
                &platform_state,
                StorageProperty::AllowUnentitledResumePoints,
                true,
                None,
            )
            .await;
        }
        let grant = test_for_req_cap(pre_setup, &cap, AppBehaviour::AllGranted).await;
        assert_eq!(grant, Ok(()));
    }

    /*
     * In this test we are testing the cap xrn:firebolt:capability:privacy:test1
     * In firebolt methods we have configured x-allow-value to be true for privacy.autoUnentitledResumePoints
     * In grant policy is configured to depend on property privacy.autoUnentitledResumePoints
     * and auto update policy as always.
     *
     * During startup in the storage property, we are setting the value as false.
     *
     * As the auto apply is always, if the storage is same as that of x-allow-value, it should
     * silently grant else silently deny.
     *
     * Here storage has the differnt value as that of x-allow-value so cap should be denied.
     */
    #[tokio::test]
    async fn test_privacy_settings_in_policy_settings_always_allow_denied() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:privacy:test1".into(),
            CapabilityRole::Use,
        )];
        async fn pre_setup(_helper: Box<RippleHelper>, platform_state: PlatformState) {
            debug!("Pre setup called while testing");
            let _ = StorageManager::set_bool(
                &platform_state,
                StorageProperty::AllowUnentitledResumePoints,
                false,
                None,
            )
            .await;
        }
        let grant = test_for_req_cap(pre_setup, &cap, AppBehaviour::AllDenied).await;
        assert_eq!(grant.err().unwrap().0, DenyReason::GrantDenied);
    }

    /*
     * In this test we are testing the cap xrn:firebolt:capability:privacy:test2
     * In firebolt methods we have configured x-allow-value to be true for privacy.autoUnentitledResumePoints
     * In grant policy is configured to depend on property privacy.autoUnentitledResumePoints
     * and auto update policy as allow.
     *
     * During startup in the storage property, we are setting the value as false.
     *
     * As the auto apply is allow, if the storage is same as that of x-allow-value, it should
     * silently grant else it should get the grant following the user input and user input is set to grant.
     *
     * Here storage has differnt value as that of x-allow-value so cap should get from the user and the
     * user is configured to deny all grants.
     *
     * So the expected result is GrantDenied
     */
    #[tokio::test]
    async fn test_privacy_settings_in_policy_settings_allowed_denied() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:privacy:test2".into(),
            CapabilityRole::Use,
        )];
        async fn pre_setup(_helper: Box<RippleHelper>, platform_state: PlatformState) {
            debug!("Pre setup called while testing");
            let _ = StorageManager::set_bool(
                &platform_state,
                StorageProperty::AllowUnentitledResumePoints,
                false,
                None,
            )
            .await;
        }
        let grant = test_for_req_cap(pre_setup, &cap, AppBehaviour::AllDenied).await;
        assert_eq!(grant.err().unwrap().0, DenyReason::GrantDenied);
    }

    /*
     * In this test we are testing the cap xrn:firebolt:capability:privacy:test2
     * In firebolt methods we have configured x-allow-value to be true for privacy.autoUnentitledResumePoints
     * In grant policy is configured to depend on property privacy.autoUnentitledResumePoints
     * and auto update policy as allow.
     *
     * During startup in the storage property, we are setting the value as false.
     *
     * As the auto apply is allow, if the storage is same as that of x-allow-value, it should
     * silently grant else it should get the grant following the user input and user input is set to grant.
     *
     * Here storage has same value as that of x-allow-value so cap should be silently granted though the
     * user is configured to deny all grants.
     *
     * So the expected result is granted
     */
    #[tokio::test]
    async fn test_privacy_settings_in_policy_settings_allowed_granted() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:privacy:test2".into(),
            CapabilityRole::Use,
        )];
        async fn pre_setup(_helper: Box<RippleHelper>, platform_state: PlatformState) {
            debug!("Pre setup called while testing");
            let _ = StorageManager::set_bool(
                &platform_state,
                StorageProperty::AllowUnentitledResumePoints,
                true,
                None,
            )
            .await;
        }
        let grant = test_for_req_cap(pre_setup, &cap, AppBehaviour::AllDenied).await;
        assert_eq!(grant, Ok(()));
    }

    /*
     * In this test we are testing the cap xrn:firebolt:capability:privacy:test3
     * In firebolt methods we have configured x-allow-value to be true for privacy.autoUnentitledResumePoints
     * In grant policy is configured to depend on property privacy.autoUnentitledResumePoints
     * and auto update policy as disallow.
     *
     * During startup in the storage property, we are setting the value as true.
     *
     * As the auto apply is disallow, if the storage is different as that of x-allow-value, it should
     * silently deny else it should get the grant following the user input and user input is set to grant.
     *
     * Here storage has same value as that of x-allow-value so it gets the user's consent
     * user is configured to grant all grants.
     *
     * So the expected result is granted
     */
    #[tokio::test]
    async fn test_privacy_settings_in_policy_settings_disallowed_granted() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:privacy:test3".into(),
            CapabilityRole::Use,
        )];
        async fn pre_setup(_helper: Box<RippleHelper>, platform_state: PlatformState) {
            debug!("Pre setup called while testing");
            let _ = StorageManager::set_bool(
                &platform_state,
                StorageProperty::AllowUnentitledResumePoints,
                true,
                None,
            )
            .await;
        }
        let grant = test_for_req_cap(pre_setup, &cap, AppBehaviour::AllGranted).await;
        assert_eq!(grant, Ok(()));
    }

    /*
     * In this test we are testing the cap xrn:firebolt:capability:privacy:test3
     * In firebolt methods we have configured x-allow-value to be true for privacy.autoUnentitledResumePoints
     * In grant policy is configured to depend on property privacy.autoUnentitledResumePoints
     * and auto update policy as disallow.
     *
     * During startup in the storage property, we are setting the value as false.
     *
     * As the auto apply is disallow, if the storage is different as that of x-allow-value, it should
     * silently deny else it should get the grant following the user input and user input is set to grant.
     *
     * Here storage has differnt value as that of x-allow-value so it siliently denys the grant
     *
     * So the expected result is denied
     */
    #[tokio::test]
    async fn test_privacy_settings_in_policy_settings_disallowed_denied() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:privacy:test3".into(),
            CapabilityRole::Use,
        )];
        async fn pre_setup(_helper: Box<RippleHelper>, platform_state: PlatformState) {
            debug!("Pre setup called while testing");
            let _ = StorageManager::set_bool(
                &platform_state,
                StorageProperty::AllowUnentitledResumePoints,
                false,
                None,
            )
            .await;
        }
        let grant = test_for_req_cap(pre_setup, &cap, AppBehaviour::AllGranted).await;
        assert_eq!(grant.err().unwrap().0, DenyReason::GrantDenied);
    }

    /*
     * In this test we are testing the cap xrn:firebolt:capability:privacy:test5
     * In firebolt methods we have configured x-allow-value to be true for privacy.autoUnentitledResumePoints
     * In grant policy is configured to depend on property privacy.autoUnentitledResumePoints
     * and auto update policy as never.
     *
     * During startup in the storage property, we are setting the value as true.
     *
     * As the auto apply is never, irrespective of the privacy settings, it should
     * it should get the grant following the user input. Here user input is set to grant.
     *
     * So the expected result is granted
     */
    #[tokio::test]
    async fn test_privacy_settings_in_policy_settings_never_granted() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:privacy:test4".into(),
            CapabilityRole::Use,
        )];
        async fn pre_setup(_helper: Box<RippleHelper>, platform_state: PlatformState) {
            debug!("Pre setup called while testing");
            let _ = StorageManager::set_bool(
                &platform_state,
                StorageProperty::AllowUnentitledResumePoints,
                true,
                None,
            )
            .await;
        }
        let grant = test_for_req_cap(pre_setup, &cap, AppBehaviour::AllGranted).await;
        assert_eq!(grant, Ok(()));
    }

    /*
     * In this test we are testing the cap xrn:firebolt:capability:privacy:test5
     * In firebolt methods we have configured x-allow-value to be true for privacy.autoUnentitledResumePoints
     * In grant policy is configured to depend on property privacy.autoUnentitledResumePoints
     * and auto update policy as never.
     *
     * During startup in the storage property, we are setting the value as true.
     *
     * As the auto apply is never, irrespective of the privacy settings, it should
     * it should get the grant following the user input. Here user input is set to deny.
     *
     * So the expected result is denied
     */
    #[tokio::test]
    async fn test_privacy_settings_in_policy_settings_never_denied() {
        let cap = vec![FireboltPermission::parse_expect(
            "xrn:firebolt:capability:privacy:test5".into(),
            CapabilityRole::Use,
        )];
        async fn pre_setup(_helper: Box<RippleHelper>, platform_state: PlatformState) {
            debug!("Pre setup called while testing");
            let _ = StorageManager::set_bool(
                &platform_state,
                StorageProperty::AllowUnentitledResumePoints,
                true,
                None,
            )
            .await;
        }
        let grant = test_for_req_cap(pre_setup, &cap, AppBehaviour::AllDenied).await;
        assert_eq!(grant.err().unwrap().0, DenyReason::GrantDenied);
    }
}
