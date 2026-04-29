//! Metrics for the actor supervisor.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;

#[derive(Debug, Default)]
pub struct ActorSupervisorMetrics {
    pub actor_restarts: AtomicU64,
    pub actor_panics: AtomicU64,
    pub actor_isolations: AtomicU64,
    pub actor_permanent_failures: AtomicU64,
    pub restart_attempts: AtomicU64,
    pub successful_restarts: AtomicU64,
    pub backtrace_captures: AtomicU64,
}

impl ActorSupervisorMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_panic(&self) {
        self.actor_panics.fetch_add(1, SeqCst);
    }

    pub fn record_restart(&self) {
        self.actor_restarts.fetch_add(1, SeqCst);
        self.successful_restarts.fetch_add(1, SeqCst);
    }

    pub fn record_restart_attempt(&self) {
        self.restart_attempts.fetch_add(1, SeqCst);
    }

    pub fn record_isolation(&self) {
        self.actor_isolations.fetch_add(1, SeqCst);
    }

    pub fn record_permanent_failure(&self) {
        self.actor_permanent_failures.fetch_add(1, SeqCst);
    }

    pub fn record_backtrace_capture(&self) {
        self.backtrace_captures.fetch_add(1, SeqCst);
    }

    pub fn get_panic_count(&self) -> u64 {
        self.actor_panics.load(SeqCst)
    }

    pub fn get_restart_count(&self) -> u64 {
        self.actor_restarts.load(SeqCst)
    }

    pub fn get_isolation_count(&self) -> u64 {
        self.actor_isolations.load(SeqCst)
    }
}

pub fn emit_actor_restart_metric(instance_id: &str, attempt: u32) {
    tracing::info!(
        instance_id = %instance_id,
        attempt = %attempt,
        metric = "actor_restart",
        "Actor restart metric emitted"
    );
}

pub fn emit_actor_panic_metric(instance_id: &str, has_backtrace: bool) {
    tracing::info!(
        instance_id = %instance_id,
        has_backtrace = %has_backtrace,
        metric = "actor_panic",
        "Actor panic metric emitted"
    );
}

pub fn emit_actor_isolation_metric(instance_id: &str, total_attempts: u32) {
    tracing::info!(
        instance_id = %instance_id,
        total_attempts = %total_attempts,
        metric = "actor_isolation",
        "Actor isolation metric emitted"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_record_panic() {
        let metrics = ActorSupervisorMetrics::new();
        assert_eq!(metrics.get_panic_count(), 0);

        metrics.record_panic();
        assert_eq!(metrics.get_panic_count(), 1);

        metrics.record_panic();
        assert_eq!(metrics.get_panic_count(), 2);
    }

    #[test]
    fn metrics_record_restart() {
        let metrics = ActorSupervisorMetrics::new();
        assert_eq!(metrics.get_restart_count(), 0);

        metrics.record_restart();
        assert_eq!(metrics.get_restart_count(), 1);
    }

    #[test]
    fn metrics_record_isolation() {
        let metrics = ActorSupervisorMetrics::new();
        assert_eq!(metrics.get_isolation_count(), 0);

        metrics.record_isolation();
        assert_eq!(metrics.get_isolation_count(), 1);
    }

    #[test]
    fn metrics_thread_safe() {
        let metrics = ActorSupervisorMetrics::new();

        std::thread::scope(|s| {
            for _ in 0..10 {
                s.spawn(|| {
                    metrics.record_panic();
                    metrics.record_restart();
                    metrics.record_isolation();
                });
            }
        });

        assert_eq!(metrics.get_panic_count(), 10);
        assert_eq!(metrics.get_restart_count(), 10);
        assert_eq!(metrics.get_isolation_count(), 10);
    }
}