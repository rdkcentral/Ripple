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

use ripple_sdk::{
    api::{
        apps::CloseReason,
        device::device_request::{DeviceLaunchEvents, OnDestroyedEvent, OnLaunchedEvent},
        firebolt::fb_lifecycle_management::LifecycleManagementRequest,
        manifest::app_library::AppLibrary,
    },
    async_trait::async_trait,
    extn::{
        client::{
            extn_processor::{
                DefaultExtnStreamer, ExtnRequestProcessor, ExtnStreamProcessor, ExtnStreamer,
            }, extn_client::ExtnClient,
        },
        extn_client_message::ExtnMessage,
    }, log::debug, tokio::sync::mpsc,
};

use crate::{launcher_state::LauncherState};

#[derive(Debug)]
pub struct LauncherEventRequestProcessor {
    state: LauncherState,
    streamer: DefaultExtnStreamer,
}

impl LauncherEventRequestProcessor {
    pub fn new(state: LauncherState) -> LauncherEventRequestProcessor {
        LauncherEventRequestProcessor {
            state,
            streamer: DefaultExtnStreamer::new(),
        }
    }

    pub async fn onlaunch(state: LauncherState, event: OnLaunchedEvent) -> bool {
        let app_id = event.client.split("-").nth(1).unwrap().to_string();

        let app_manifest =
            AppLibrary::get_manifest(&state.config.app_library_state, &app_id).unwrap();

        if event.client.contains("Html-") && app_manifest.runtime == "web".to_string() {

            let request = LifecycleManagementRequest::Ready(app_id.clone());

            if let Err(e) = state.clone().send_extn_request(request).await {
                debug!("Failed to send Lifecycle.ready() event, Error {:?} ", e);
            };
        }
        true
    }

    pub async fn ondestroyed(state: LauncherState, event: OnDestroyedEvent) -> bool {
        let app_id = event.client.split("-").nth(1).unwrap().to_string();

        let app_manifest =
            AppLibrary::get_manifest(&state.config.app_library_state, &app_id).unwrap();



        let entry = state.clone().app_launcher_state.get_app_by_id(&app_id);
        println!("App status : web app : {:?} : status : {:?}",app_id,entry);

        if entry.is_none() {
            debug!(
                "launcher : app_id={} Not found",app_id
            );
        }
        else {
            if event.client.contains("Html-") && app_manifest.runtime == "web".to_string() {
                let request =
                    LifecycleManagementRequest::Close(app_id.clone(), CloseReason::AppNotReady);
    
                if let Err(e) = state.clone().send_extn_request(request).await {
                    debug!("Failed to send Lifecycle.close() event, Error {:?} ", e);
                };
            }
        }
        true
    }
}

impl ExtnStreamProcessor for LauncherEventRequestProcessor {
    type VALUE = DeviceLaunchEvents;
    type STATE = LauncherState;

    fn get_state(&self) -> Self::STATE {
        self.state.clone()
    }

    fn receiver(&mut self) -> mpsc::Receiver<ExtnMessage> {
        self.streamer.receiver()
    }

    fn sender(&self) -> mpsc::Sender<ExtnMessage> {
        self.streamer.sender()
    }
}

#[async_trait]
impl ExtnRequestProcessor for LauncherEventRequestProcessor {
    fn get_client(&self) -> ExtnClient {
        self.state.extn_client.clone()
    }
    async fn process_request(
        state: Self::STATE,
        _msg: ExtnMessage,
        extracted_message: Self::VALUE,
    ) -> bool {
        match extracted_message {
            DeviceLaunchEvents::Onlaunched(val) => {
                println!("Launcher : event recived : {:?}", val);
                Self::onlaunch(state.clone(), val).await;
            }
            DeviceLaunchEvents::Ondestroyed(val) => {
                println!("Launcher : event recived : {:?}", val);
                Self::ondestroyed(state.clone(), val).await;
            }
        };
        true
    }
}
