use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListenRequest {
    pub listen: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ListenerResponse {
    pub listening: bool,
    pub event: String,
}
