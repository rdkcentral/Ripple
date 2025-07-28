#[macro_export]
macro_rules! sync_read_lock {
    ($lock:expr) => {
        match $lock.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("RwLock poisoned, recovering with inner value");
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
                eprintln!("RwLock poisoned during write, recovering with inner value");
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
#[macro_export]
macro_rules! async_write_unlock {
    ($guard:ident) => {
        drop($guard);
    };
}

#[macro_export]
macro_rules! async_write {
    ($lock:expr, |$guard:ident| $body:block) => {{
        let arc = $lock.clone();
        let mut $guard = arc.write().await;
        let result = (|| $body)();
        drop($guard); // Explicitly drop to release before await points, if any
        result
    }};
}

#[macro_export]
macro_rules! async_write_async {
    ($lock:expr, |$guard:ident| $body:block) => {{
        let arc = $lock.clone();
        async move {
            let mut $guard = arc.write().await;
            $body
        }
        .await
    }};
}
#[macro_export]
macro_rules! async_read_async {
    ($lock:expr, |$guard:ident| $body:block) => {{
        let arc = $lock.clone();
        async move {
            let $guard = arc.read().await;
            $body
        }
        .await
    }};
}

#[macro_export]
macro_rules! async_read {
    ($lock:expr, |$guard:ident| $body:block) => {{
        let arc = $lock.clone();
        let $guard = arc.read().await;
        let result = (|| $body)();
        drop($guard);
        result
    }};
}

pub type AsyncRwLock<T> = tokio::sync::RwLock<T>;
pub type SyncRwLock<T> = std::sync::RwLock<T>;

pub type SyncShared<T> = std::sync::Arc<SyncRwLock<T>>;
pub type AsyncShared<T> = std::sync::Arc<AsyncRwLock<T>>;
