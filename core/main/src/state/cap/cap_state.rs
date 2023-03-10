use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

use ripple_sdk::api::firebolt::{
    fb_capabilities::{DenyReason, DenyReasonWithCap, FireboltCap},
    fb_openrpc::CapabilitySet,
};

#[derive(Clone, Debug, Default)]
pub struct GenericCapState {
    supported: Arc<RwLock<HashSet<String>>>,
    // it consumes less memory and operations to store not_available vs available
    not_available: Arc<RwLock<HashSet<String>>>,
}

impl GenericCapState {
    pub fn ingest_supported(&self, request: Vec<FireboltCap>) {
        let mut supported = self.supported.write().unwrap();
        supported.extend(
            request
                .iter()
                .map(|a| a.as_str())
                .collect::<HashSet<String>>(),
        )
    }

    pub fn ingest_availability(&self, request: Vec<FireboltCap>, is_available: bool) {
        let mut not_available = self.not_available.write().unwrap();
        for cap in request {
            if is_available {
                not_available.remove(&cap.as_str());
            } else {
                not_available.insert(cap.as_str());
            }
        }
    }

    pub fn check_supported(&self, request: Vec<FireboltCap>) -> Result<(), DenyReasonWithCap> {
        let supported = self.supported.read().unwrap();
        let mut not_supported: Vec<FireboltCap> = Vec::new();
        for cap in request {
            if !supported.contains(&cap.as_str()) {
                not_supported.push(cap.clone());
            }
        }
        if not_supported.len() > 0 {
            return Err(DenyReasonWithCap::new(
                DenyReason::Unsupported,
                not_supported,
            ));
        }
        Ok(())
    }

    pub fn check_available(&self, request: Vec<FireboltCap>) -> Result<(), DenyReasonWithCap> {
        let not_available = self.not_available.read().unwrap();
        let mut result: Vec<FireboltCap> = Vec::new();
        for cap in request {
            if not_available.contains(&cap.as_str()) {
                result.push(cap.clone())
            }
        }
        if result.len() > 0 {
            return Err(DenyReasonWithCap::new(DenyReason::Unavailable, result));
        }
        Ok(())
    }

    pub fn check_all(&self, request: CapabilitySet) -> Result<(), DenyReasonWithCap> {
        let caps = request.clone().get_caps();
        if let Err(e) = self.check_supported(caps.clone()) {
            return Err(e);
        }

        self.check_available(caps)
    }
}
