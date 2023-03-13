use std::sync::{Arc, RwLock};

use ripple_sdk::{
    tokio::sync::{mpsc, oneshot},
    uuid::Uuid,
};
use serde::{Deserialize, Serialize};

use super::container_manager::ContainerProperties;

#[derive(Debug, Clone)]
pub struct ContainerRequest {
    pub method: ContainerMethod,
    pub resp_tx: Arc<RwLock<Option<oneshot::Sender<ContainerResponse>>>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ResultType {
    None,
    Uuid(Uuid),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ContainerError {
    General,
    NotFound,
    NotSupported,
    IoError,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ContainerResponse {
    pub status: Result<ResultType, ContainerError>,
}

#[allow(dead_code)]
pub enum ContainerMessageType {
    Request,
    Response,
}

#[derive(Debug, Clone)]
pub enum ContainerEvent {
    Added(ContainerProperties),
    Focused(Option<ContainerProperties>, Option<ContainerProperties>),
    Removed(ContainerProperties),
}

#[derive(Debug, Clone)]
pub enum ContainerMethod {
    Add(ContainerProperties),
    Remove(String),
    BringToFront(String),
    SendToBack(String),
    SetVisible(String, bool),
    EventRegistration(mpsc::Sender<ContainerEvent>),
}

pub struct ContainerMessage {
    pub message_type: ContainerMessageType,
    pub request: Option<ContainerRequest>,
    pub response: Option<ContainerResponse>,
}

impl std::fmt::Display for ContainerMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "foo")
    }
}
