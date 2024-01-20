use std::{
    sync::atomic::{AtomicI64, AtomicUsize, Ordering},
    time::Duration,
};

use rust_extensions::date_time::{AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds};

use super::OneSecondMetric;

pub struct ConnectionStatistics {
    pub connected: DateTimeAsMicroseconds,
    pub disconnected: AtomicDateTimeAsMicroseconds,
    pub last_send_moment: AtomicDateTimeAsMicroseconds,
    pub last_receive_moment: AtomicDateTimeAsMicroseconds,
    pub total_received: AtomicUsize,
    pub total_sent: AtomicUsize,
    pub received_per_sec: OneSecondMetric,
    pub sent_per_sec: OneSecondMetric,
    pub pending_to_send_buffer_size: AtomicUsize,
    pub ping_start: AtomicI64,
    pub round_trip_micros: AtomicI64,
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
            received_per_sec: OneSecondMetric::new(),
            sent_per_sec: OneSecondMetric::new(),
            pending_to_send_buffer_size: AtomicUsize::new(0),
            ping_start: AtomicI64::new(0),
            round_trip_micros: AtomicI64::new(0),
        }
    }

    pub fn update_read_amount(&self, amount: usize) {
        let now = DateTimeAsMicroseconds::now();
        self.last_receive_moment.update(now);
        self.received_per_sec.increase(amount);
        self.total_received.fetch_add(amount, Ordering::SeqCst);
    }

    pub fn update_sent_amount(&self, amount: usize) {
        let now = DateTimeAsMicroseconds::now();
        self.last_send_moment.update(now);
        self.sent_per_sec.increase(amount);
        self.total_sent.fetch_add(amount, Ordering::SeqCst);
    }

    pub fn one_second_tick(&self) {
        self.received_per_sec.one_second_tick();
        self.sent_per_sec.one_second_tick();
    }

    pub fn disconnect(&self) {
        self.disconnected.update(DateTimeAsMicroseconds::now());
    }

    pub fn set_ping_start(&self) {
        let now = DateTimeAsMicroseconds::now();
        self.ping_start
            .store(now.unix_microseconds, Ordering::SeqCst);
    }

    pub fn update_ping_pong_statistic(&self) {
        let ping_start = self.ping_start.load(Ordering::Relaxed);

        if ping_start == 0 {
            return;
        }

        let ping_start = DateTimeAsMicroseconds::new(ping_start);

        let now = DateTimeAsMicroseconds::now();

        let duration = now.duration_since(ping_start).as_positive_or_zero();
        self.round_trip_micros
            .store(duration.as_micros() as i64, Ordering::SeqCst);
    }

    pub fn get_ping_pong_duration(&self) -> Option<Duration> {
        let round_trip_micros = self.round_trip_micros.load(Ordering::Relaxed);
        if round_trip_micros == 0 {
            return None;
        }

        Some(Duration::from_micros(round_trip_micros as u64))
    }
}
