use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::SeqCst);
    }

    #[must_use]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Default)]
pub struct Histogram {
    count: AtomicU64,
    sum: AtomicU64,
}

impl Histogram {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, value: u64) {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.sum.fetch_add(value, Ordering::SeqCst);
    }

    #[must_use]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn sum(&self) -> u64 {
        self.sum.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn average(&self) -> u64 {
        let cnt = self.count();
        if cnt == 0 {
            return 0;
        }
        self.sum() / cnt
    }
}

#[derive(Debug, Default)]
pub struct HotReloadMetrics {
    pub reload_success_total: Counter,
    pub reload_error_total: Counter,
    pub reload_latency_ms: Histogram,
}

impl HotReloadMetrics {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_reload_success(&self, duration_ms: u64) {
        self.reload_success_total.inc();
        self.reload_latency_ms.record(duration_ms);
    }

    pub fn record_reload_error(&self) {
        self.reload_error_total.inc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_initial_value_is_zero() {
        let counter = Counter::new();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn counter_inc_increments_value() {
        let counter = Counter::new();
        counter.inc();
        assert_eq!(counter.get(), 1);
        counter.inc();
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn histogram_initial_values_are_zero() {
        let hist = Histogram::new();
        assert_eq!(hist.count(), 0);
        assert_eq!(hist.sum(), 0);
        assert_eq!(hist.average(), 0);
    }

    #[test]
    fn histogram_record_increments_count_and_sum() {
        let hist = Histogram::new();
        hist.record(100);
        hist.record(200);
        assert_eq!(hist.count(), 2);
        assert_eq!(hist.sum(), 300);
        assert_eq!(hist.average(), 150);
    }

    #[test]
    fn histogram_average_returns_zero_when_empty() {
        let hist = Histogram::new();
        assert_eq!(hist.average(), 0);
    }

    #[test]
    fn hot_reload_metrics_initial_all_zero() {
        let metrics = HotReloadMetrics::new();
        assert_eq!(metrics.reload_success_total.get(), 0);
        assert_eq!(metrics.reload_error_total.get(), 0);
        assert_eq!(metrics.reload_latency_ms.count(), 0);
    }

    #[test]
    fn hot_reload_metrics_record_reload_success() {
        let metrics = HotReloadMetrics::new();
        metrics.record_reload_success(50);
        metrics.record_reload_success(100);
        assert_eq!(metrics.reload_success_total.get(), 2);
        assert_eq!(metrics.reload_latency_ms.count(), 2);
        assert_eq!(metrics.reload_latency_ms.sum(), 150);
        assert_eq!(metrics.reload_latency_ms.average(), 75);
    }

    #[test]
    fn hot_reload_metrics_record_reload_error() {
        let metrics = HotReloadMetrics::new();
        metrics.record_reload_error();
        metrics.record_reload_error();
        metrics.record_reload_error();
        assert_eq!(metrics.reload_error_total.get(), 3);
        assert_eq!(metrics.reload_success_total.get(), 0);
    }
}