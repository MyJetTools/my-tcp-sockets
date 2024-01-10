use std::sync::atomic::AtomicI64;

pub struct ThreadAmount {
    value: AtomicI64,
}

impl ThreadAmount {
    pub fn new() -> Self {
        Self {
            value: AtomicI64::new(0),
        }
    }
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

pub struct ThreadsStatistics {
    pub read_threads: ThreadAmount,
    pub write_threads: ThreadAmount,
    pub ping_threads: ThreadAmount,

    pub connections_objects: ThreadAmount,
}

impl ThreadsStatistics {
    pub fn new() -> Self {
        Self {
            read_threads: ThreadAmount::new(),
            write_threads: ThreadAmount::new(),
            ping_threads: ThreadAmount::new(),
            connections_objects: ThreadAmount::new(),
        }
    }
}
