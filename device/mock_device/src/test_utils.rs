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

use std::collections::HashMap;

use ripple_sdk::{
    async_channel::{unbounded, Receiver},
    extn::{
        client::extn_sender::ExtnSender,
        extn_id::{ExtnClassId, ExtnId},
        ffi::ffi_message::CExtnMessage,
    },
};

pub fn extn_sender_web_socket_mock_server() -> (ExtnSender, Receiver<CExtnMessage>) {
    let (tx, receiver) = unbounded();
    let sender = ExtnSender::new(
        tx,
        ExtnId::new_channel(ExtnClassId::Device, "mock_device".to_owned()),
        vec![],
        vec!["web_socket.mock_server".to_owned()],
        Some(HashMap::from([(
            "mock_data_file".to_owned(),
            "examples/device-mock-data/mock-device.json".to_owned(),
        )])),
    );

    (sender, receiver)
}

// pub fn extn_sender_jsonrpsee() -> (ExtnSender, Receiver<CExtnMessage>) {
//     let (tx, receiver) = unbounded();
//     let sender = ExtnSender::new(
//         tx,
//         ExtnId::new_channel(ExtnClassId::Device, "mock_device".to_owned()),
//         vec!["web_socket.mock_server".to_owned()],
//         vec!["json_rpsee".to_owned()],
//         None,
//     );

//     (sender, receiver)
// }
