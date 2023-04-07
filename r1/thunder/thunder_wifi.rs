use crate::{
    client::thunder_client::{ThunderClient, ThunderParams, ThunderResponseMessage},
    gateway::thunder_plugin::ThunderPlugin::Wifi,
    service::thunder_service_resolver::ThunderDelegate,
    service::thunder_service_resolver::ThunderOperator,
};
use async_trait::async_trait;
use dab_core::{
    message::{DabError, DabRequest, DabResponsePayload},
    model::wifi::{
        AccessPoint, AccessPointList, AccessPointRequest, WifiDabService, WifiRequest,
        WifiSecurityMode,
    },
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info, info_span, Instrument, Span};

pub struct WifiDelegate;

#[async_trait]
impl ThunderDelegate for WifiDelegate {
    async fn handle(&self, request: DabRequest, client: Box<ThunderClient>) {
        let service = Box::new(ThunderWifiService { client: client });
        match request.payload.as_wifi_request().unwrap() {
            WifiRequest::Scan => {
                let span = request.req_span("WifiRequest::Scan");
                service.scan(request).instrument(span).await;
            }
            WifiRequest::Connect(connect) => {
                let span = request.req_span("WifiRequest::Connect");
                service.connect(request, connect).instrument(span).await;
            }
        }
    }
}

pub struct ThunderWifiService {
    pub client: Box<ThunderClient>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InternetStatus {
    connected_to_internet: bool,
    success: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScanResult {
    success: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThunderSSID {
    ssid: String,
    security: u32,
    signal_strength: i32,
    frequency: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SSIDEventResponse {
    more_data: bool,
    ssids: Vec<ThunderSSID>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectResult {
    success: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_camel_case_types, non_snake_case)]
struct WifiStateChanged {
    state: u32,
    isLNF: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectedSSIDResult {
    ssid: String,
    security: String,
    signal_strength: String,
    frequency: String,
}

impl ConnectedSSIDResult {
    fn to_access_point(self: Box<Self>) -> AccessPoint {
        AccessPoint {
            ssid: self.ssid.clone(),
            security_mode: wifi_security_mode_from_u32(
                self.security.clone().parse::<u32>().unwrap(),
            ),
            signal_strength: self.signal_strength.clone().parse::<i32>().unwrap(),
            frequency: ThunderSSID::to_frequency(self.frequency),
        }
    }
}

impl ThunderSSID {
    fn to_access_point(self: Box<Self>) -> AccessPoint {
        AccessPoint {
            ssid: self.ssid.clone(),
            security_mode: wifi_security_mode_from_u32(self.security),
            signal_strength: self.signal_strength,
            frequency: self.frequency,
        }
    }

    fn to_frequency(s: String) -> f32 {
        match s.parse::<f32>() {
            Ok(v) => v,
            Err(_e) => {
                error!("invalid frequency {}", _e);
                0f32
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThunderWifiConnectRequest {
    pub ssid: String,
    pub passphrase: String,
    pub security_mode: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThunderWifiScanRequest {
    pub incremental: bool,
}

impl ThunderWifiConnectRequest {
    fn from_access_point_request(access_point_request: AccessPointRequest) -> Self {
        Self {
            ssid: access_point_request.ssid.clone(),
            passphrase: access_point_request.passphrase.clone(),
            security_mode: wifi_security_mode_to_u32(access_point_request.security),
        }
    }
}

pub fn wifi_security_mode_to_u32(v: WifiSecurityMode) -> u32 {
    match v {
        WifiSecurityMode::Wep64 => 1,
        WifiSecurityMode::Wep128 => 2,
        WifiSecurityMode::WpaPskTkip => 3,
        WifiSecurityMode::WpaPskAes => 4,
        WifiSecurityMode::Wpa2PskTkip => 5,
        WifiSecurityMode::Wpa2PskAes => 6,
        WifiSecurityMode::WpaEnterpriseTkip => 7,
        WifiSecurityMode::WpaEnterpriseAes => 8,
        WifiSecurityMode::Wpa2EnterpriseTkip => 9,
        WifiSecurityMode::Wpa2EnterpriseAes => 10,
        WifiSecurityMode::Wpa2Psk => 11,
        WifiSecurityMode::Wpa2Enterprise => 12,
        WifiSecurityMode::Wpa3PskAes => 13,
        WifiSecurityMode::Wpa3Sae => 14,
        WifiSecurityMode::None => 0,
    }
}

pub fn wifi_security_mode_from_u32(v: u32) -> WifiSecurityMode {
    match v {
        1 => WifiSecurityMode::Wep64,
        2 => WifiSecurityMode::Wep128,
        3 => WifiSecurityMode::WpaPskTkip,
        4 => WifiSecurityMode::WpaPskAes,
        5 => WifiSecurityMode::Wpa2PskTkip,
        6 => WifiSecurityMode::Wpa2PskAes,
        7 => WifiSecurityMode::WpaEnterpriseTkip,
        8 => WifiSecurityMode::WpaEnterpriseAes,
        9 => WifiSecurityMode::Wpa2EnterpriseTkip,
        10 => WifiSecurityMode::Wpa2EnterpriseAes,
        11 => WifiSecurityMode::Wpa2Psk,
        12 => WifiSecurityMode::Wpa2Enterprise,
        13 => WifiSecurityMode::Wpa3PskAes,
        14 => WifiSecurityMode::Wpa3Sae,
        0 | _ => WifiSecurityMode::None,
    }
}

impl ThunderWifiService {
    async fn start_scan(client: Box<ThunderClient>, span: Span) -> ScanResult {
        let start_scan = Wifi.method("startScan");
        let request = ThunderWifiScanRequest { incremental: false };
        let start_scan_response = client
            .clone()
            .call_thunder(
                &start_scan,
                Some(ThunderParams::Json(
                    serde_json::to_string(&request).unwrap(),
                )),
                Some(span.clone()),
            )
            .await;
        serde_json::from_value(start_scan_response.message).unwrap()
    }

    async fn wait_for_thunder_ssids(
        client: Box<ThunderClient>,
        request: DabRequest,
        parent_span: Span,
    ) {
        let span = info_span!(
            parent: &parent_span,
            "subscribing to wifi ssid scan thunder events"
        );
        let client = client.clone();
        let (s, mut r) = tokio::sync::mpsc::channel::<ThunderResponseMessage>(32);
        let unsub_client = client.clone();
        client
            .subscribe(Wifi.callsign(), "onAvailableSSIDs", None, Some(span), s)
            .await;
        // spawn a thread that handles all scan events, handle the success and error events
        tokio::spawn(async move {
            if let Some(m) = r.recv().await {
                let mut list = Vec::new();
                let ssid_response: SSIDEventResponse = serde_json::from_value(m.message).unwrap();
                let mut dedup = Vec::new();
                for ssid in ssid_response.ssids {
                    let check_ssid = ssid.ssid.clone();
                    if !dedup.contains(&check_ssid) {
                        list.push(Box::new(ssid).to_access_point());
                        dedup.push(check_ssid);
                    }
                }

                list.sort_by(|a, b| b.signal_strength.cmp(&a.signal_strength));

                let success_response =
                    Ok(DabResponsePayload::WifiScanListResponse(AccessPointList {
                        list,
                    }));
                request.respond_and_log(success_response);
                unsub_client
                    .unsubscribe(Wifi.callsign(), "onAvailableSSIDs")
                    .await;
            }
        });
    }

    async fn connect_to_wifi(
        client: Box<ThunderClient>,
        access_point_request: AccessPointRequest,
        span: Span,
    ) -> ConnectResult {
        let connect_req = Wifi.method("connect");
        let connect_wifi_request =
            ThunderWifiConnectRequest::from_access_point_request(access_point_request);
        let connect_response = client
            .clone()
            .call_thunder(
                &connect_req,
                Some(ThunderParams::Json(
                    serde_json::to_string(&connect_wifi_request).unwrap(),
                )),
                Some(span.clone()),
            )
            .await;
        serde_json::from_value(connect_response.message).unwrap()
    }

    async fn wait_for_connection(
        client: Box<ThunderClient>,
        request: DabRequest,
        parent_span: Span,
    ) {
        let span = info_span!(
            parent: &parent_span,
            "subscribing to wifi ssid scan thunder events"
        );
        let sub_client = client.clone();
        let sub_span = span.clone();
        let (s, mut r) = tokio::sync::mpsc::channel::<ThunderResponseMessage>(32);
        let unsub_client = client.clone();
        sub_client
            .subscribe(Wifi.callsign(), "onWIFIStateChanged", None, Some(span), s)
            .await;
        // spawn a thread that handles all connection pairing events, handle the success and error events
        tokio::spawn(async move {
            while let Some(m) = r.recv().await {
                let wifi_state_changed: WifiStateChanged =
                    serde_json::from_value(m.message).unwrap();
                info!("Wifi statechanged={}", wifi_state_changed.state);
                match wifi_state_changed.state {
                    5 => {
                        let success_response = Ok(DabResponsePayload::WifiConnectSuccessResponse(
                            ThunderWifiService::get_connected_ssid(client, sub_span.clone()).await,
                        ));

                        request.respond_and_log(success_response);
                        unsub_client
                            .unsubscribe(Wifi.callsign(), "onWIFIStateChanged")
                            .await;
                        break;
                    }
                    6 => {
                        error!("Failure to connect to SSID");
                        request.respond_and_log(Err(DabError::OsError));
                        unsub_client
                            .unsubscribe(Wifi.callsign(), "onWIFIStateChanged")
                            .await;
                        break;
                    }
                    _ => {}
                }
            }
        });
    }

    async fn get_connected_ssid(client: Box<ThunderClient>, span: Span) -> AccessPoint {
        let get_connected_ssid = Wifi.method("getConnectedSSID");

        let get_connected_ssid_response = client
            .clone()
            .call_thunder(&get_connected_ssid, None, Some(span.clone()))
            .await;
        let get_connected_ssid_response: ConnectedSSIDResult =
            serde_json::from_value(get_connected_ssid_response.message).unwrap();
        Box::new(get_connected_ssid_response).to_access_point()
    }
}

#[async_trait]
impl WifiDabService for ThunderWifiService {
    async fn scan(self: Box<Self>, request: DabRequest) -> Box<Self> {
        let parent_span = match request.parent_span.clone() {
            Some(s) => s,
            None => info_span!("scanning wifi"),
        };
        let scan_result =
            ThunderWifiService::start_scan(self.client.clone(), parent_span.clone()).await;
        if scan_result.success {
            ThunderWifiService::wait_for_thunder_ssids(
                self.client.clone(),
                request,
                parent_span.clone(),
            )
            .await;
        } else {
            error!("Wifi scan failure")
        }
        self
    }

    async fn connect(
        self: Box<Self>,
        request: DabRequest,
        connect_request: AccessPointRequest,
    ) -> Box<Self> {
        let parent_span = match request.parent_span.clone() {
            Some(s) => s,
            None => info_span!("connecting wifi"),
        };
        let scan_result = ThunderWifiService::connect_to_wifi(
            self.client.clone(),
            connect_request,
            parent_span.clone(),
        )
        .await;
        if scan_result.success {
            if let Err(_) = timeout(
                Duration::from_secs(180),
                ThunderWifiService::wait_for_connection(
                    self.client.clone(),
                    request,
                    parent_span.clone(),
                ),
            )
            .await
            {
                error!("Timeout passed")
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        client::thunder_client::{
            ThunderCallMessage, ThunderResponseMessage, ThunderSubscribeMessage,
        },
        test::{mock_thunder_controller::CustomHandler, test_utils::ThunderDelegateTest},
        util::channel_util::oneshot_send_and_log,
    };
    use dab_core::message::{DabRequestPayload, DabResponse};
    use serde_json::json;
    use std::collections::HashMap;
    use tokio::sync::oneshot::error::RecvError;

    #[tokio::test]
    async fn test_thunder_wifi() {
        let mut custom_handler = CustomHandler {
            custom_request_handler: HashMap::new(),
            custom_subscription_handler: HashMap::new(),
        };
        let payload = DabRequestPayload::Wifi(WifiRequest::Scan);
        let handle_function = |msg: ThunderCallMessage| {
            let response = json!({"success" : true});
            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };

        // Scan Result ???
        let event_handler = |_msg: &ThunderSubscribeMessage| {
            let mut ssids = Vec::new();
            let ssid = ThunderSSID {
                ssid: String::from("ssid1"),
                security: 1,
                signal_strength: 100,
                frequency: 1000.0,
            };
            info!("tunderSSID{:?}", ssid);
            ssids.push(ssid);
            let result = SSIDEventResponse {
                more_data: false,
                ssids,
            };
            let remote_status_event = json!(result);
            ThunderResponseMessage::call(remote_status_event)
        };
        custom_handler.custom_subscription_handler.insert(
            String::from("org.rdk.Wifi".to_owned() + &"onAvailableSSIDs".to_owned()),
            event_handler,
        );
        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.Wifi".to_owned() + &"startScan".to_owned()),
            handle_function,
        );
        let delegate = Box::new(WifiDelegate);

        let validator = |res: Result<DabResponse, RecvError>| {
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Ok { .. }));
                if let Ok(_payload) = dab_response {}
            }
        };

        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, Some(custom_handler), Box::new(validator))
            .await;

        test_thunder_wifi_connect().await;

        for n in 0..15 {
            let sec = wifi_security_mode_from_u32(n);
            info!("security mode {:?}", sec);
        }
    }

    async fn test_thunder_wifi_connect() {
        let mut custom_handler = CustomHandler {
            custom_request_handler: HashMap::new(),
            custom_subscription_handler: HashMap::new(),
        };
        let payload = DabRequestPayload::Wifi(WifiRequest::Connect(AccessPointRequest {
            ssid: String::from("Matrix"),
            passphrase: String::from("abcd"),
            security: WifiSecurityMode::WpaPskAes,
        }));
        let handle_function = |msg: ThunderCallMessage| {
            let response = json!({"success" : true});
            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };
        let event_handler = |_msg: &ThunderSubscribeMessage| {
            let result = WifiStateChanged {
                state: 5,
                isLNF: false,
            };
            let remote_status_event = json!(result);
            ThunderResponseMessage::call(remote_status_event)
        };
        custom_handler.custom_subscription_handler.insert(
            String::from("org.rdk.Wifi".to_owned() + &"onWIFIStateChanged".to_owned()),
            event_handler,
        );
        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.Wifi".to_owned() + &"connect".to_owned()),
            handle_function,
        );

        let handle_function2 = |msg: ThunderCallMessage| {
            let ssid = ConnectedSSIDResult {
                ssid: String::from("ssid1"),
                security: String::from("1"),
                signal_strength: String::from("100"),
                frequency: String::from("1000.0"),
            };
            info!("Connected SSID{:?}", ssid);
            let response = json!(ssid);
            let resp_msg = ThunderResponseMessage::call(response.clone());
            oneshot_send_and_log(msg.callback, resp_msg, "StatusReturn")
        };
        custom_handler.custom_request_handler.insert(
            String::from("org.rdk.Wifi".to_owned() + &"getConnectedSSID".to_owned()),
            handle_function2,
        );

        let delegate = Box::new(WifiDelegate);
        let validator = |res: Result<DabResponse, RecvError>| {
            if let Ok(dab_response) = res {
                assert!(matches!(dab_response, Ok { .. }));
                if let Ok(_payload) = dab_response {}
            }
        };
        let delegate_test = ThunderDelegateTest::new(delegate);
        delegate_test
            .start_test(payload, Some(custom_handler), Box::new(validator))
            .await;
    }
}
