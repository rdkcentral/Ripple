use futures::future::ok;
use ripple_sdk::{tokio::sync::{mpsc::Receiver, oneshot}, api::{firebolt::fb_metrics::MetricsContext, device::{device_request::DeviceRequest, device_info_request::DeviceInfoRequest}}, log::info, utils::channel_utils::oneshot_send_and_log, extn::extn_client_message::ExtnRequest};

use crate::{state::platform_state::PlatformState, firebolt::handlers::device_rpc::get_ll_mac_addr};

#[derive(Debug, Clone)]
pub enum MetricsContextErr {
    NoContextAvailable,
}
#[derive(Debug)]
pub struct ContextRequest {
    pub callback: oneshot::Sender<MetricsContextResponse>,
}
#[derive(Debug)]
pub struct MetricsContextResponse {
    pub metrics_context: Option<MetricsContext>,
}
#[derive(Debug)]
pub enum MetricsRequest {
    GetContext(ContextRequest),
    Refresh,
}
#[derive(Debug)]
pub enum MetricsResponse {
    MetricsContext(MetricsContextResponse),
}
pub struct MetricsManager {
    /*
    channel to interact with clients
    */
    metrics_request_channel: Receiver<MetricsRequest>,
    /*
    the actual context
    */
    metrics_context: Option<MetricsContext>,
    /*
    platform state
    */
    platform_state: PlatformState,
}
impl MetricsManager {
    pub fn new(
        metrics_request_channel: Receiver<MetricsRequest>,
        platform_state: PlatformState,
    ) -> MetricsManager {
        MetricsManager {
            metrics_request_channel: metrics_request_channel,
            metrics_context: None,
            platform_state,
        }
    }

    pub async fn start(&mut self) {
        info!("MetricsManager: entry");
        self.refresh_context().await;
        while let Some(req) = self.metrics_request_channel.recv().await {
            self.process_request(req).await;
        }
        info!("MetricsManager: exit");
    }

    async fn process_request(&mut self, request: MetricsRequest) {
        match request {
            MetricsRequest::GetContext(get_context_request) => {
                self.get_context(get_context_request).await
            }
            MetricsRequest::Refresh => self.refresh_context().await,
        }
    }

    async fn get_context(&self, request: ContextRequest) {
        oneshot_send_and_log(
            request.callback,
            MetricsContextResponse {
                metrics_context: self.metrics_context.clone(),
            },
            "Metrics",
        );
    }

    async fn refresh_context(&mut self) -> () {
        /*
        For now, get them all
        Once we move to more DOP, hopefully this can be simplified
        */
        let mac_address = MetricsManager::get_value(
        Some(get_ll_mac_addr(self.platform_state.clone()).await.unwrap())
        );

        
        let device_model = MetricsManager::get_value(
            // self.platform_state
            //     .services
            //     .send_dab(DabRequestPayload::Device(DeviceRequest::Model))
            //     .await
            //     .unwrap()
            //     .as_string(),
        );

        let dist_session = self
            .platform_state
            .services
            .send_dab(DabRequestPayload::Distributor(DistributorRequest::Session))
            .await
            .unwrap()
            .as_dist_session();
        /*TODO : Enable the following code when PlatformState is available in metrics manager*/
        /*
        // clear the cached distributor session
        // self.platform_state.app_auth_sessions.clear_device_auth_session().await;
         */
        let (account, device_id, session_id) = match dist_session {
            Some(session) => (
                MetricsManager::get_value(session.account_id),
                MetricsManager::get_value(session.device_id),
                MetricsManager::get_value(session.id),
            ),
            None => (
                "no.account.set".to_string(),
                "no.device_id.set".to_string(),
                "no.session_id.set".to_string(),
            ),
        };

        let language =
            match StorageManager::get_string(&self.platform_state, StorageProperty::Language).await
            {
                Ok(resp) => resp,
                Err(_) => "no.language.set".to_string(),
            };

        let mut os = FireboltSemanticVersion::new(0, 0, 0, "".to_string());
        os.minor += 7;
        let a_str: String = format!("Firebolt OS v{}.{}.{}", os.major, os.minor, os.patch);
        os.readable = a_str;
        let os_ver = match self
            .platform_state
            .services
            .send_dab(DabRequestPayload::Device(DeviceRequest::Version))
            .await
        {
            Ok(val) => MetricsManager::get_value(val.as_string()),
            Err(_) => "no.os.ver.set".to_string(),
        };

        let device_name =
            match StorageManager::get_string(&self.platform_state, StorageProperty::DeviceName)
                .await
            {
                Ok(resp) => resp,
                Err(_) => "no.device.name.set".to_string(),
            };

        let mut mutant = match &self.metrics_context {
            Some(thing) => thing.clone(),
            None => MetricsContext::new(),
        };

        mutant.set(MetricsContextField::mac_address, mac_address.clone());
        mutant.set(MetricsContextField::device_id, device_id);
        mutant.set(MetricsContextField::account_id, account);
        mutant.set(MetricsContextField::device_language, language);
        mutant.set(MetricsContextField::device_model, device_model);
        mutant.set(MetricsContextField::device_name, device_name);
        /*
        TODO, add correct timezone when it becomes available
        */
        mutant.set(MetricsContextField::device_timezone, "0".to_string());
        mutant.set(MetricsContextField::os_ver, os_ver);
        mutant.set(MetricsContextField::platform, "ripple".to_string());
        mutant.set(MetricsContextField::serial_number, mac_address.clone());
        mutant.set(MetricsContextField::session_id, session_id.clone());

        let wrapped_metrics_context = Some(mutant);
        self.metrics_context = wrapped_metrics_context;
    }
    fn get_value(maybe_value: Option<String>) -> String {
        match maybe_value {
            Some(v) => v.clone(),
            None => String::from(""),
        }
    }
}

