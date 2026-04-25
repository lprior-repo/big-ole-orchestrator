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

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }

    pub fn incr(&self) {
        self.value.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Debug, Default)]
pub struct SpawnSupervisorMetrics {
    pub spawns_successful: Counter,
    pub spawns_failed: Counter,
    pub health_checks_performed: Counter,
    pub health_checks_failed: Counter,
    pub zombies_detected: Counter,
    pub respawns: Counter,
    pub dispatch_errors: Counter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_new() {
        let counter = Counter::new();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn counter_increment() {
        let counter = Counter::new();
        counter.incr();
        assert_eq!(counter.get(), 1);
        counter.incr();
        counter.incr();
        assert_eq!(counter.get(), 3);
    }

    #[test]
    fn spawn_supervisor_metrics_default() {
        let metrics = SpawnSupervisorMetrics::default();
        assert_eq!(metrics.spawns_successful.get(), 0);
        assert_eq!(metrics.spawns_failed.get(), 0);
        assert_eq!(metrics.health_checks_performed.get(), 0);
        assert_eq!(metrics.health_checks_failed.get(), 0);
        assert_eq!(metrics.zombies_detected.get(), 0);
        assert_eq!(metrics.respawns.get(), 0);
        assert_eq!(metrics.dispatch_errors.get(), 0);
    }
}