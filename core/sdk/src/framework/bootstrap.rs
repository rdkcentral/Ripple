use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use log::debug;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::utils::error::RippleError;

pub struct Bootstrap<S: Clone> {
    state: S,
}

impl<S: Clone> Bootstrap<S> {
    pub fn new(s: S) -> Bootstrap<S> {
        Bootstrap { state: s }
    }

    pub async fn step(&self, s: impl Bootstep<S>) -> Result<&Self, RippleError> {
        debug!(">>>Starting Bootstep {}<<<", s.get_name());
        if let Err(e) = s.setup(self.state.clone()).await {
            return Err(e);
        }

        debug!("---Successful Bootstep {}---", s.get_name());
        Ok(self)
    }
}

#[async_trait]
pub trait Bootstep<S: Clone> {
    fn get_name(&self) -> String;
    async fn setup(&self, s: S) -> Result<(), RippleError>;
}

/// This struct can be used during bootstrap process where we initialize the channel and setup sender.
/// And later start the receiver in a different step of the bootstrap.
///
/// # Examples
/// ```
/// // pre init
///  let (tx, tr) = mpsc::channel::<RpcRequest>::(32);
///  let transient_channel = TransientChannel::new(tx,tr);
/// // step 1
///  transient_channel.get_sender() // gets tx you can call this any number of times
/// // step x
///  transient_channel.get_receiver().recv().expect("can be called only once").await
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
        if tr.is_some() {
            return Ok(tr.take().unwrap());
        }

        Err(RippleError::InvalidOutput)
    }
}
