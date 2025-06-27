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

use log::{error, trace};
use tokio::sync::{mpsc, oneshot};

pub async fn mpsc_send_and_log<T: std::fmt::Debug>(
    tx: &mpsc::Sender<T>,
    message: T,
    channel_id: &str,
) {
    println!("@@@NNA....mpsc send fn");
    match tx.send(message).await {
        Ok(_) => {
            trace!("Successfully sent message through mpsc channel")
        }
        Err(e) => {
            error!(
                "Failed to send message {:?} through mpsc channel {}",
                e, channel_id
            )
        }
    }
}

pub fn oneshot_send_and_log<T: std::fmt::Debug>(
    tx: oneshot::Sender<T>,
    message: T,
    channel_id: &str,
) {
    match tx.send(message) {
        Ok(_) => {
            trace!("Successfully sent message through oneshot channel",)
        }
        Err(e) => {
            error!(
                "Failed to send message {:?} through oneshot channel {}",
                e, channel_id
            )
        }
    }
}
