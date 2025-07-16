use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared, thread-safe, async-aware mutable reference
pub type SharedRW<T> = std::sync::Arc<tokio::sync::RwLock<T>>;
