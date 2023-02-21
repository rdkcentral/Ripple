use async_trait::async_trait;
use log::debug;

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
