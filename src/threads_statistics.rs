use std::sync::atomic::AtomicUsize;

pub struct ThreadsStatistics {
    pub read_threads: AtomicUsize,
    pub write_threads: AtomicUsize,
    pub ping_threads: AtomicUsize,
}

impl ThreadsStatistics {
    pub fn new() -> Self {
        Self {
            read_threads: AtomicUsize::new(0),
            write_threads: AtomicUsize::new(0),
            ping_threads: AtomicUsize::new(0),
        }
    }

    pub fn increase_read_threads(&self) {
        self.read_threads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn decrease_read_threads(&self) {
        self.read_threads
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn increase_write_threads(&self) {
        self.write_threads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn decrease_write_threads(&self) {
        self.write_threads
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn increase_ping_threads(&self) {
        self.ping_threads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn decrease_ping_threads(&self) {
        self.ping_threads
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn get_read_threads(&self) -> usize {
        self.read_threads.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn get_write_threads(&self) -> usize {
        self.write_threads.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn get_ping_threads(&self) -> usize {
        self.ping_threads.load(std::sync::atomic::Ordering::SeqCst)
    }
}
