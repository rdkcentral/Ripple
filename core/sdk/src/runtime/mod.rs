use std::time::Duration;

// use crate::{runtime::task_registry::{TrackedRequestId, TASKS}, RequestId};

// tokio::task_local! {
//     static CURRENT_REQUEST_ID: TrackedRequestId;
// }

// /// run your request handler “inside” the request-id context
// pub async fn with_request_id<R, F>(req_id: impl Into<TrackedRequestId>, fut: F) -> R
// where
//     F: std::future::Future<Output = R>,
// {
//     let rid = req_id.into();
//     CURRENT_REQUEST_ID.scope(rid, fut).await
// }

// pub struct RequestScope {
//     rid: TrackedRequestId,
//     grace: Duration,
// }

// impl RequestScope {
//     pub fn new(rid: impl Into<TrackedRequestId>) -> Self {
//         Self { rid: rid.into(), grace: Duration::from_millis(200) }
//     }
//     pub fn with_grace(mut self, d: Duration) -> Self { self.grace = d; self }
//     pub async fn run<F, R>(self, fut: F) -> R
//     where
//         F: std::future::Future<Output = R>,
//     {
//         let rid = self.rid.clone();
//         let grace = self.grace;
//         // enter task-local scope
//         with_request_id(rid.clone(), async move {
//             let out = fut.await;
//             // cleanup *only this request’s* tasks
//             TASKS.finish_request(&rid, grace).await;
//             out
//         }).await
//     }
// }

pub mod task_registry;
