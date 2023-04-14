use crate::{
    client::thunder_client::{ThunderClient, ThunderParams, ThunderResponseMessage},
    gateway::thunder_plugin::ThunderPlugin::RemoteControl,
    service::thunder_service_resolver::ThunderOperator,
};
use async_trait::async_trait;
use dab_core::{
    message::{DabError, DabRequest, DabResponsePayload},
    model::accessory::{
        AccessoryDeviceListResponse, AccessoryDeviceResponse, AccessoryListRequest,
        AccessoryPairRequest, AccessoryProtocol, AccessoryProtocolListType, AccessoryService,
        AccessoryType,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, info_span, warn};

pub struct ThunderRemoteAccessoryService {
    pub client: Box<ThunderClient>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteStatusEvent {
    status: RemoteStatus,
}

impl RemoteStatusEvent {
    pub fn get_list_response(self: Box<Self>) -> AccessoryDeviceListResponse {
        let mut remote_list = Vec::new();
        let (new_self, opt_protocol) = self.get_protocol();
        let protocol = opt_protocol.unwrap();
        let remote_data_list = new_self.status.remote_data;
        for i in remote_data_list {
            remote_list.push(RemoteStatusEvent::get_accessory_device_response(
                i,
                protocol.clone(),
            ))
        }
        AccessoryDeviceListResponse { list: remote_list }
    }

    pub fn get_protocol(self: Box<Self>) -> (Box<Self>, Option<AccessoryProtocol>) {
        let status = self.status.net_type.clone();
        (
            self,
            match status {
                0 => Some(AccessoryProtocol::RF4CE),
                1 => Some(AccessoryProtocol::BluetoothLE),
                _ => None,
            },
        )
    }

    pub fn get_accessory_device_response(
        remote_data: RemoteData,
        protocol: AccessoryProtocol,
    ) -> AccessoryDeviceResponse {
        AccessoryDeviceResponse {
            _type: AccessoryType::Remote,
            make: remote_data.make,
            model: remote_data.model,
            protocol,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteStatus {
    net_type: u32,
    pairing_state: String,
    remote_data: Vec<RemoteData>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteData {
    make: String,
    model: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemotePairRequest {
    net_type: u32,
    timeout: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteListRequest {
    net_type: u32,
}

impl ThunderRemoteAccessoryService {
    pub async fn unsubscribe_pairing(client: Box<ThunderClient>) {
        client
            .unsubscribe(RemoteControl.callsign(), "onStatus")
            .await;
    }

    pub fn get_net_type(protocol: AccessoryProtocol) -> u32 {
        match protocol {
            AccessoryProtocol::BluetoothLE => 1,
            AccessoryProtocol::RF4CE => 0,
        }
    }

    pub fn get_net_type_list(protocol: AccessoryProtocolListType) -> u32 {
        match protocol {
            AccessoryProtocolListType::BluetoothLE => 1,
            AccessoryProtocolListType::RF4CE | AccessoryProtocolListType::All => 0,
        }
    }

    pub fn get_accessory_response(
        protocol: AccessoryProtocol,
        make: String,
        model: String,
    ) -> DabResponsePayload {
        let accessory_response = AccessoryDeviceResponse {
            _type: AccessoryType::Remote,
            make,
            model,
            protocol: protocol,
        };

        DabResponsePayload::AccessoryDeviceResponse(accessory_response)
    }
}

#[async_trait]
impl AccessoryService for ThunderRemoteAccessoryService {
    async fn pair(
        self: Box<Self>,
        dab_request: DabRequest,
        pair_request_opt: Option<AccessoryPairRequest>,
    ) -> Box<Self> {
        let pair_request = pair_request_opt.unwrap_or_default();
        let parent_span = match dab_request.parent_span.clone() {
            Some(s) => s,
            None => info_span!("pairing accessory"),
        };
        let protocol = pair_request.protocol;
        let remote_pair_request = RemotePairRequest {
            net_type: ThunderRemoteAccessoryService::get_net_type(protocol.clone()),
            timeout: pair_request.timeout,
        };
        let request_method = RemoteControl.method("startPairing");
        let start_pairing_response = self
            .client
            .clone()
            .call_thunder(
                &request_method,
                Some(ThunderParams::Json(
                    serde_json::to_string(&remote_pair_request).unwrap(),
                )),
                Some(parent_span.clone()),
            )
            .await;
        info!("{}", start_pairing_response.message);

        if start_pairing_response.message["success"].as_bool().unwrap() {
            let span = info_span!(parent: &parent_span, "subscribing to remote thunder events");
            let client = self.client.clone();
            let (s, mut r) = tokio::sync::mpsc::channel::<ThunderResponseMessage>(32);
            let unsub_client = client.clone();
            client
                .subscribe(RemoteControl.callsign(), "onStatus", None, Some(span), s)
                .await;
            // spawn a thread that handles all remote pairing events, handle the success and error events
            tokio::spawn(async move {
                while let Some(m) = r.recv().await {
                    let remote_status_event: RemoteStatusEvent =
                        serde_json::from_value(m.message).unwrap();
                    let pairing_status = remote_status_event.status.pairing_state.clone();
                    match pairing_status.as_str() {
                        "CONFIGURATION_COMPLETE" => {
                            info!("successfully paired");
                            let success_accessory_response: DabResponsePayload;
                            if remote_status_event.status.remote_data.len() > 0 {
                                let remote = remote_status_event.status.remote_data.get(0).unwrap();
                                success_accessory_response =
                                    ThunderRemoteAccessoryService::get_accessory_response(
                                        protocol,
                                        remote.make.clone(),
                                        remote.model.clone(),
                                    );
                            } else {
                                warn!("No Remote info");
                                success_accessory_response =
                                    ThunderRemoteAccessoryService::get_accessory_response(
                                        protocol,
                                        "".into(),
                                        "".into(),
                                    )
                            }
                            dab_request.respond_and_log(Ok(success_accessory_response));
                            ThunderRemoteAccessoryService::unsubscribe_pairing(unsub_client).await;
                            break;
                        }
                        "FAILED" => {
                            error!("Failure to pair remote");
                            let error_response = Err(DabError::OsError);
                            dab_request.respond_and_log(error_response);
                            ThunderRemoteAccessoryService::unsubscribe_pairing(unsub_client).await;
                            break;
                        }
                        _ => {
                            info!("Remote Pairing Status {}", pairing_status)
                        }
                    }
                }
            });
        } else {
            error!("Failure to start pairing for remote");
            let error_response = Err(DabError::OsError);
            dab_request.respond_and_log(error_response);
        }
        self
    }

    async fn list(
        self: Box<Self>,
        dab_request: DabRequest,
        list_request_opt: Option<AccessoryListRequest>,
    ) -> Box<Self> {
        let parent_span = match dab_request.parent_span.clone() {
            Some(s) => s,
            None => info_span!("pairing accessory"),
        };
        let list_request = list_request_opt.unwrap_or_default();
        let protocol = list_request
            .protocol
            .unwrap_or(AccessoryProtocolListType::All);
        let remote_pair_request = RemoteListRequest {
            net_type: ThunderRemoteAccessoryService::get_net_type_list(protocol.clone()),
        };
        let request_method = RemoteControl.method("getNetStatus");
        let list_response = self
            .client
            .clone()
            .call_thunder(
                &request_method,
                Some(ThunderParams::Json(
                    serde_json::to_string(&remote_pair_request).unwrap(),
                )),
                Some(parent_span.clone()),
            )
            .await;
        info!("{}", list_response.message);
        let remote_status_event: RemoteStatusEvent =
            serde_json::from_value(list_response.message).unwrap();
        let remote_event = Box::new(remote_status_event);
        let success_response = Ok(DabResponsePayload::AccessoryDeviceListResponse(
            remote_event.get_list_response(),
        ));
        dab_request.respond_and_log(success_response);
        self
    }
}
