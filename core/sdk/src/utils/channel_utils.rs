use log::{error, trace};
use tokio::sync::{mpsc, oneshot};

pub async fn mpsc_send_and_log<T: std::fmt::Debug>(
    tx: &mpsc::Sender<T>,
    message: T,
    channel_id: &str,
) {
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
