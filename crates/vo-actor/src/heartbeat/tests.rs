//! Tests for the heartbeat module.

use super::*;
use crate::probe::Probe;

struct MockProbe {
    id: ProbeId,
    healthy: bool,
}

impl MockProbe {
    fn new(healthy: bool) -> Self {
        Self {
            id: ProbeId::new(),
            healthy,
        }
    }
}

#[async_trait::async_trait]
impl Probe for MockProbe {
    async fn check(&self) -> Result<ProbeResult, ProbeError> {
        if self.healthy {
            Ok(ProbeResult {
                probe_id: self.id,
                status: ProbeStatus::Healthy,
                latency_ms: 10,
                consecutive_failures: 0,
                last_check_ms: 0,
                message: Some("healthy".to_string()),
            })
        } else {
            Ok(ProbeResult {
                probe_id: self.id,
                status: ProbeStatus::Unhealthy,
                latency_ms: 0,
                consecutive_failures: 1,
                last_check_ms: 0,
                message: Some("unhealthy".to_string()),
            })
        }
    }

    fn probe_id(&self) -> ProbeId {
        self.id
    }
}

#[tokio::test]
async fn test_heartbeat_watcher_registers_actor() {
    let watcher = HeartbeatWatcher::with_defaults();
    watcher.register_actor("actor-1".to_string()).await;

    let states = watcher.actor_states.read().await;
    assert!(states.contains_key("actor-1"));
}

#[tokio::test]
async fn test_heartbeat_watcher_unregisters_actor() {
    let watcher = HeartbeatWatcher::with_defaults();
    watcher.register_actor("actor-1".to_string()).await;
    watcher.unregister_actor("actor-1").await;

    let states = watcher.actor_states.read().await;
    assert!(!states.contains_key("actor-1"));
}

#[tokio::test]
async fn test_actor_health_state_records_healthy() {
    let mut state = ActorHealthState::new();
    state.record_healthy();
    assert_eq!(state.consecutive_failures, 0);
    assert!(state.last_healthy.is_some());
}

#[tokio::test]
async fn test_actor_health_state_records_failure() {
    let mut state = ActorHealthState::new();
    state.record_failure();
    assert_eq!(state.consecutive_failures, 1);
    state.record_failure();
    assert_eq!(state.consecutive_failures, 2);
    assert!(state.last_check.is_some());
}

#[tokio::test]
async fn test_actor_health_state_healthy_resets_failures() {
    let mut state = ActorHealthState::new();
    state.record_failure();
    state.record_failure();
    assert_eq!(state.consecutive_failures, 2);
    state.record_healthy();
    assert_eq!(state.consecutive_failures, 0);
}

#[tokio::test]
async fn test_shutdown_callback_invoked_on_threshold() {
    let config = HeartbeatWatcherConfig {
        failure_threshold: 2,
        ..Default::default()
    };

    let shutdown_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let shutdown_called_clone = shutdown_called.clone();

    let callback: ShutdownCallback = Box::new(move |actor_id| {
        assert_eq!(actor_id, "test-actor");
        shutdown_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    });

    let watcher = HeartbeatWatcher::new(config.clone()).with_shutdown_callback(callback);

    watcher.register_actor("test-actor".to_string()).await;

    watcher.record_failure("test-actor").await;
    assert!(!shutdown_called.load(std::sync::atomic::Ordering::SeqCst));

    watcher.record_failure("test-actor").await;
    assert!(shutdown_called.load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test]
async fn test_config_default_values() {
    let config = HeartbeatWatcherConfig::default();
    assert_eq!(config.check_interval, Duration::from_secs(30));
    assert_eq!(config.failure_threshold, 3);
    assert_eq!(config.graceful_shutdown_timeout, Duration::from_secs(30));
}
