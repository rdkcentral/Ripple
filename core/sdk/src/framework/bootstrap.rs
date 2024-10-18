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

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use log::debug;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::utils::error::RippleError;

use super::RippleResponse;

pub struct Bootstrap<S: Clone> {
    state: S,
}

impl<S: Clone> Bootstrap<S> {
    pub fn new(s: S) -> Bootstrap<S> {
        Bootstrap { state: s }
    }

    pub async fn step(&self, s: impl Bootstep<S>) -> Result<&Self, RippleError> {
        debug!(">>>Starting Bootstep {}<<<", s.get_name());
        s.setup(self.state.clone()).await?;

        debug!("---Successful Bootstep {}---", s.get_name());
        Ok(self)
    }
}

#[async_trait]
pub trait Bootstep<S: Clone> {
    fn get_name(&self) -> String;
    async fn setup(&self, s: S) -> RippleResponse;
}

/// This struct can be used during bootstrap process where we initialize the channel and setup sender.
/// And later start the receiver in a different step of the bootstrap.
///
/// # Examples
/// ```
/// // pre init
/// use ripple_sdk::api::gateway::rpc_gateway_api::RpcRequest;
/// use ripple_sdk::framework::bootstrap::TransientChannel;
///  let (tx, tr) = tokio::sync::mpsc::channel::<RpcRequest>(32);
///  let transient_channel = TransientChannel::new(tx,tr);
/// // step 1
///  let tx = transient_channel.get_sender(); // gets tx you can call this any number of times
/// // step x
///  let rx = transient_channel.get_receiver().expect("can be called only once");
/// ```
#[derive(Debug, Clone)]
pub struct TransientChannel<T> {
    tx: Sender<T>,
    tr: Arc<RwLock<Option<Receiver<T>>>>,
}

impl<T> TransientChannel<T> {
    pub fn new(tx: Sender<T>, tr: Receiver<T>) -> TransientChannel<T> {
        Self {
            tx,
            tr: Arc::new(RwLock::new(Some(tr))),
        }
    }

    pub fn get_sender(&self) -> Sender<T> {
        self.tx.clone()
    }

    pub fn get_receiver(&self) -> Result<Receiver<T>, RippleError> {
        let mut tr = self.tr.write().unwrap();

        if let Some(receiver) = tr.take() {
            Ok(receiver)
        } else {
            Err(RippleError::InvalidAccess)
        }
    }
}
