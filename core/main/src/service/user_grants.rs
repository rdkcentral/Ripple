use serde::{Deserialize, Serialize};

pub struct UserGrants {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChallengeResponse {
    pub granted: bool,
}
