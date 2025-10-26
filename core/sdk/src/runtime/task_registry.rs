use crate::log::debug;
use dashmap::DashMap;
use futures::future::{AbortHandle, Abortable};
use once_cell::sync::Lazy;
use std::{sync::Arc, time::Duration};
use tokio::{task::JoinHandle, time};

use crate::RequestId; // ← your existing type

#[derive(Clone, Debug)]
pub struct TrackedRequestId(pub RequestId);

struct Group {
    // id -> AbortHandle
    tasks: DashMap<u64, AbortHandle>,
}

impl Group {
    fn new() -> Self {
        Self {
            tasks: DashMap::new(),
        }
    }
}

pub struct TaskRegistry {
    groups: DashMap<RequestId, Arc<Group>>,
    next_id: std::sync::atomic::AtomicU64,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            groups: DashMap::new(),
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    fn group(&self, rid: &RequestId) -> Arc<Group> {
        self.groups
            .entry(rid.clone())
            .or_insert_with(|| Arc::new(Group::new()))
            .clone()
    }

    /// Spawn a task tracked under a request id.
    /// Returns the spawned JoinHandle for callers that want to .await/.abort directly.
    pub fn spawn_for<F>(&self, rid: &RequestId, fut: F) -> JoinHandle<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let g = self.group(rid);
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        println!("spawn_for:Registering task for request {}, {}", id, rid);

        // Wrap in Abortable so we can cancel via AbortHandle kept in the map.
        let (abort_handle, abort_reg) = AbortHandle::new_pair();
        g.tasks.insert(id, abort_handle);

        let g_for_cleanup = g.clone();

        let join = tokio::spawn(async move {
            // Run the task (may end normally or via abort)
            let _ = Abortable::new(fut, abort_reg).await;
            // On completion (ok or aborted), remove from the registry

            println!("spawn_for: Task finished or aborted {}", id);
            g_for_cleanup.tasks.remove(&id);
        });

        join
    }

    /// Abort all tasks for this request and wait briefly for cleanup to finish.
    pub async fn finish_request(&self, rid: &RequestId, grace: Duration) {
        if let Some((_, g)) = self.groups.remove(rid) {
            // Abort everyone
            for entry in g.tasks.iter() {
                entry.value().abort();
            }
            debug!("finish_request: cleaning up  {} tasks", g.tasks.len());
            // Best-effort drain: wait until the bucket empties or we hit the deadline.
            let deadline = time::Instant::now() + grace;
            loop {
                if g.tasks.is_empty() || time::Instant::now() >= deadline {
                    break;
                }
                time::sleep(Duration::from_millis(10)).await;
            }

            // If anything remains (very unlikely), just clear.
            g.tasks.clear();
        }
        debug_runtime_task_counts();
    }
}

use tokio::runtime::Handle;

fn debug_runtime_task_counts() {
    if let Ok(handle) = Handle::try_current() {
        let metrics = handle.metrics();

        debug!(
            "spawn: [RUNTIME] Total tasks = {} | Workers= {} | Alive tasks = {}",
            metrics.num_alive_tasks(), // total tasks alive
            metrics.num_workers(),     // idle tasks
            metrics.num_alive_tasks(), // spawn_blocking threads
        );
    } else {
        println!("[RUNTIME] No current runtime");
    }
}

pub static TASKS: Lazy<TaskRegistry> = Lazy::new(TaskRegistry::new);

pub struct FinishOnDrop {
    rid: RequestId,
    grace: Duration,
}

impl FinishOnDrop {
    pub fn new(rid: RequestId) -> Self {
        Self::new_with_deadline(rid, Duration::from_millis(300))
    }
    pub fn new_with_deadline(rid: RequestId, grace: Duration) -> Self {
        Self { rid, grace }
    }
}

impl Drop for FinishOnDrop {
    fn drop(&mut self) {
        let rid = self.rid.clone();
        let grace = self.grace;
        tokio::spawn(async move {
            TASKS.finish_request(&rid, grace).await;
        });
    }
}
impl From<RequestId> for TrackedRequestId {
    fn from(rid: RequestId) -> Self {
        TrackedRequestId(rid)
    }
}
