#[macro_export]
macro_rules! sync_read_lock {
    ($lock:expr) => {
        match $lock.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("⚠️ RwLock poisoned, recovering with inner value");
                poisoned.into_inner()
            }
        }
    };
}

#[macro_export]
macro_rules! sync_write_lock {
    ($lock:expr) => {
        match $lock.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("⚠️ RwLock poisoned during write, recovering with inner value");
                poisoned.into_inner()
            }
        }
    };
}

#[macro_export]
macro_rules! async_read_lock {
    ($lock:expr) => {
        $lock.read().await
    };
}

#[macro_export]
macro_rules! async_write_lock {
    ($lock:expr) => {
        $lock.write().await
    };
}
pub type AsyncRwLock<T> = tokio::sync::RwLock<T>;
pub type SyncRwLock<T> = std::sync::RwLock<T>;

pub type SyncShared<T> = std::sync::Arc<SyncRwLock<T>>;
pub type AsyncShared<T> = std::sync::Arc<AsyncRwLock<T>>;

#[macro_export]
macro_rules! clone_and_read_lock {
    (
        $platform_state:expr, $field:ident $(=> $binding:ident)?
    ) => {{
        let cloned = $platform_state.clone();
        let guard = cloned.$field.read().await;
        $(let $binding = guard;)*
        guard
    }};
}
