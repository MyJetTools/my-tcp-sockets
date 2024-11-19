use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
pub struct OneSecondMetric {
    intermediary_value: AtomicUsize,
    value: AtomicUsize,
}

impl OneSecondMetric {
    pub fn new() -> Self {
        Self {
            intermediary_value: AtomicUsize::new(0),
            value: AtomicUsize::new(0),
        }
    }

    pub fn increase(&self, delta: usize) {
        self.intermediary_value.fetch_add(delta, Ordering::SeqCst);
    }

    pub fn one_second_tick(&self) {
        let result = self.intermediary_value.swap(0, Ordering::SeqCst);
        self.value.store(result, Ordering::SeqCst);
    }

    pub fn get_value(&self) -> usize {
        self.value.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_basic_case() {
        let metrics = OneSecondMetric::new();

        metrics.increase(5);

        metrics.increase(3);

        metrics.one_second_tick();

        assert_eq!(8, metrics.get_value());
    }
}
