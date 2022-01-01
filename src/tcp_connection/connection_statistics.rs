use std::sync::atomic::{AtomicUsize, Ordering};

use rust_extensions::date_time::{AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds};

pub struct ConnectionStatistics {
    pub connected: DateTimeAsMicroseconds,
    pub disconnected: AtomicDateTimeAsMicroseconds,
    pub last_send_moment: AtomicDateTimeAsMicroseconds,
    pub last_receive_moment: AtomicDateTimeAsMicroseconds,
    pub total_received: AtomicUsize,
    pub total_sent: AtomicUsize,
    pub received_per_sec: AtomicUsize,
    pub sent_per_sec: AtomicUsize,
}

impl ConnectionStatistics {
    pub fn new() -> Self {
        let now = DateTimeAsMicroseconds::now();
        Self {
            connected: DateTimeAsMicroseconds::new(now.unix_microseconds),
            disconnected: AtomicDateTimeAsMicroseconds::new(now.unix_microseconds),
            last_send_moment: AtomicDateTimeAsMicroseconds::now(),
            last_receive_moment: AtomicDateTimeAsMicroseconds::now(),
            total_received: AtomicUsize::new(0),
            total_sent: AtomicUsize::new(0),
            received_per_sec: AtomicUsize::new(0),
            sent_per_sec: AtomicUsize::new(0),
        }
    }

    pub fn update_read_amount(&self, amount: usize) {
        let now = DateTimeAsMicroseconds::now();
        self.last_receive_moment.update(now);
        self.received_per_sec.fetch_add(amount, Ordering::SeqCst);
        self.total_received.fetch_add(amount, Ordering::SeqCst);
    }

    pub fn update_sent_amount(&self, amount: usize) {
        let now = DateTimeAsMicroseconds::now();
        self.last_send_moment.update(now);
        self.sent_per_sec.fetch_add(amount, Ordering::SeqCst);
        self.total_sent.fetch_add(amount, Ordering::SeqCst);
    }

    pub fn one_second_tick(&self) {
        todo!("Implement")
    }

    pub fn disconnect(&self) {
        self.disconnected.update(DateTimeAsMicroseconds::now());
    }
}
