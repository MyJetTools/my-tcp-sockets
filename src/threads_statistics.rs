use std::sync::atomic::AtomicI64;

#[derive(Default)]
pub struct ThreadAmount {
    value: AtomicI64,
}

impl ThreadAmount {
    pub fn increase(&self) {
        self.value.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn decrease(&self) {
        self.value.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn get(&self) -> i64 {
        self.value.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[derive(Default)]
pub struct ThreadsStatistics {
    pub read_threads: ThreadAmount,
    pub write_threads: ThreadAmount,
    pub ping_threads: ThreadAmount,
    pub connections_objects: ThreadAmount,
}
