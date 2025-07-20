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
