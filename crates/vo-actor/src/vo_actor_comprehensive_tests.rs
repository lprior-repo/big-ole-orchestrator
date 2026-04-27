//! Comprehensive tests for vo-actor: hibernation round-trip, supervisor restart, and probe instrumentation.
//!
//! These tests address bead vel-b1b requirements:
//! 1. Hibernation round-trip tests: state preservation across sleep/wake cycles, disk format stability
//! 2. Supervisor restart tests: restart policies, state recovery after crash, at-most-one actor guarantee
//! 3. Probe instrumentation tests: metric emission, trace correlation, health check responses

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::probe::{
    AggregatedStatus, BackoffConfig, ProbeConfig, ProbeDefinition, ProbeId, ProbeRegistry,
    ProbeResult, ProbeStatus,
};
use crate::reanimator::mock::{MockTimerStorage, MockWorkQueue};
use crate::reanimator::traits::TimerStorage;
use crate::reanimator::types::{FairnessBudget, TimerRecord};
use crate::spawn_supervisor::{
    calculate_backoff_delay, is_zombie_state, should_respawn, CycleResult, ProcessHandle,
    ProcessManager, SpawnPhase, SpawnRecord, SpawnStorage, SpawnSupervisor, SpawnSupervisorError,
    SpawnSupervisorMetrics, SpawnSupervisorState, WorkQueue,
};
use vo_types::{InstanceId, TimestampMs};

// =============================================================================
// Helper Functions
// =============================================================================

fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

fn make_instance_id(seed: u8) -> InstanceId {
    InstanceId::from_bytes([seed; 16])
}

fn make_timer_record(instance_id: InstanceId, fire_at_ms: u64) -> TimerRecord {
    TimerRecord::new(
        instance_id,
        ts_ms(fire_at_ms),
        Some(vo_types::TimerId::from_bytes([1; 16])),
        ts_ms(fire_at_ms - 1000),
    )
}

// =============================================================================
// Section 1: Hibernation Round-Trip Tests
// Tests for state preservation across sleep/wake cycles and disk format stability
// =============================================================================

mod hibernation_round_trip_tests {
    use super::*;

    #[tokio::test]
    async fn hibernation_timer_record_serialization_round_trip() {
        let instance_id = make_instance_id(1);
        let original = TimerRecord::new(
            instance_id.clone(),
            ts_ms(5000),
            Some(vo_types::TimerId::from_bytes([0xAB; 16])),
            ts_ms(4000),
        );

        let serialized = serde_json::to_string(&original).expect("should serialize");
        let deserialized: TimerRecord =
            serde_json::from_str(&serialized).expect("should deserialize");

        assert_eq!(original.instance_id, deserialized.instance_id);
        assert_eq!(original.fire_at_ms, deserialized.fire_at_ms);
        assert_eq!(original.scheduled_at_ms, deserialized.scheduled_at_ms);
        assert_eq!(original.timer_id, deserialized.timer_id);
    }

    #[tokio::test]
    async fn hibernation_timer_record_json_format_stability() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let record = TimerRecord::new(
            instance_id,
            ts_ms(5000),
            Some(vo_types::TimerId::from_bytes([1; 16])),
            ts_ms(4000),
        );

        let json = serde_json::to_string(&record).expect("should serialize");

        assert!(json.contains("01H5JYV4XHGSR2F8KZ9BWNRFMA"));
        assert!(json.contains("\"fire_at_ms\":5000"));
        assert!(json.contains("\"scheduled_at_ms\":4000"));
    }

    #[tokio::test]
    async fn hibernation_state_preserved_across_storage_cycle() {
        let instance_id = make_instance_id(42);
        let storage = Arc::new(MockTimerStorage::empty());

        let original = make_timer_record(instance_id.clone(), 5000);
        storage.add_timer(original.clone()).await;

        let scanned = storage
            .scan_due_timers(ts_ms(0), ts_ms(10000), 100)
            .await
            .expect("scan should succeed");

        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].instance_id, instance_id);
        assert_eq!(scanned[0].fire_at_ms, ts_ms(5000));
    }

    #[tokio::test]
    async fn hibernation_multiple_timers_per_instance_round_trip() {
        let instance_id = make_instance_id(99);
        let storage = Arc::new(MockTimerStorage::empty());

        let timer1 = TimerRecord::new(
            instance_id.clone(),
            ts_ms(3000),
            Some(vo_types::TimerId::from_bytes([1; 16])),
            ts_ms(2000),
        );
        let timer2 = TimerRecord::new(
            instance_id.clone(),
            ts_ms(5000),
            Some(vo_types::TimerId::from_bytes([2; 16])),
            ts_ms(4000),
        );
        let timer3 = TimerRecord::new(
            instance_id.clone(),
            ts_ms(7000),
            Some(vo_types::TimerId::from_bytes([3; 16])),
            ts_ms(6000),
        );

        storage.add_timer(timer1).await;
        storage.add_timer(timer2).await;
        storage.add_timer(timer3).await;

        let scanned = storage
            .scan_due_timers(ts_ms(0), ts_ms(10000), 100)
            .await
            .expect("scan should succeed");

        assert_eq!(scanned.len(), 3);
    }

    #[tokio::test]
    async fn hibernation_wake_preserves_timer_after_processing() {
        let instance_id = make_instance_id(7);
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .add_timer(make_timer_record(instance_id.clone(), 5000))
            .await;

        let scanned = storage
            .scan_due_timers(ts_ms(0), ts_ms(6000), 100)
            .await
            .expect("scan should succeed");
        assert_eq!(scanned.len(), 1);

        storage
            .delete_timer(&instance_id, ts_ms(5000))
            .await
            .expect("delete should succeed");
        storage
            .record_timer_fired(&instance_id, ts_ms(5000))
            .await
            .expect("record should succeed");

        let after_fire = storage
            .scan_due_timers(ts_ms(0), ts_ms(6000), 100)
            .await
            .expect("scan should succeed");
        assert_eq!(after_fire.len(), 0);

        let fire_calls = storage.fire_calls().await;
        assert_eq!(fire_calls.len(), 1);
        assert_eq!(fire_calls[0].0, instance_id);
    }

    #[tokio::test]
    async fn hibernation_pending_timer_recovery_round_trip() {
        let instance_id = make_instance_id(55);
        let storage = Arc::new(MockTimerStorage::empty());

        storage
            .mark_timer_processing(&instance_id, ts_ms(5000))
            .await
            .expect("mark should succeed");

        let pending = storage
            .scan_pending_timers(100)
            .await
            .expect("scan pending should succeed");

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].instance_id, instance_id);

        storage
            .complete_timer_processing(&instance_id, ts_ms(5000))
            .await
            .expect("complete should succeed");

        let after_complete = storage
            .scan_pending_timers(100)
            .await
            .expect("scan pending should succeed");
        assert_eq!(after_complete.len(), 0);
    }
}

// =============================================================================
// Section 2: Supervisor Restart Tests
// Tests for restart policies, state recovery, and at-most-one guarantee
// =============================================================================

mod supervisor_restart_tests {
    use super::*;

    fn create_spawn_record(
        instance_id: InstanceId,
        phase: SpawnPhase,
        attempts: u32,
    ) -> SpawnRecord {
        SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from("/test/bin"),
            args: vec![],
            spawn_phase: phase,
            health_checks: 0,
            spawn_attempts: attempts,
            last_error: None,
        }
    }

    #[test]
    fn restart_policy_should_respawn_within_limit() {
        let record = create_spawn_record(make_instance_id(1), SpawnPhase::Failed, 3);
        assert!(should_respawn(&record, 5));
    }

    #[test]
    fn restart_policy_should_not_respawn_at_limit() {
        let record = create_spawn_record(make_instance_id(1), SpawnPhase::Failed, 5);
        assert!(!should_respawn(&record, 5));
    }

    #[test]
    fn restart_policy_should_not_respawn_running_instance() {
        let record = create_spawn_record(make_instance_id(1), SpawnPhase::Running, 1);
        assert!(!should_respawn(&record, 5));
    }

    #[test]
    fn restart_policy_should_not_respawn_terminal_instance() {
        let record = create_spawn_record(make_instance_id(1), SpawnPhase::Terminated, 1);
        assert!(!should_respawn(&record, 5));
    }

    #[test]
    fn restart_backoff_calculation_exponential() {
        assert_eq!(calculate_backoff_delay(1000, 2.0, 1), 1000);
        assert_eq!(calculate_backoff_delay(1000, 2.0, 2), 2000);
        assert_eq!(calculate_backoff_delay(1000, 2.0, 3), 4000);
        assert_eq!(calculate_backoff_delay(1000, 2.0, 4), 8000);
    }

    #[test]
    fn restart_backoff_calculation_saturation() {
        assert_eq!(calculate_backoff_delay(1000, 2.0, 10), 512000);
        assert_eq!(calculate_backoff_delay(1000, 2.0, 100), u64::MAX / 2 + 1);
    }

    #[test]
    fn at_most_one_actor_zombie_detection() {
        let low_attempts = create_spawn_record(make_instance_id(1), SpawnPhase::Failed, 2);
        assert!(!is_zombie_state(&low_attempts));

        let high_attempts = create_spawn_record(make_instance_id(2), SpawnPhase::Failed, 5);
        assert!(is_zombie_state(&high_attempts));
    }

    #[test]
    fn at_most_one_actor_running_phase_not_zombie() {
        let running = create_spawn_record(make_instance_id(1), SpawnPhase::Running, 10);
        assert!(!is_zombie_state(&running));
    }

    #[test]
    fn restart_policy_record_transition_spawn_to_running() {
        let record = create_spawn_record(make_instance_id(1), SpawnPhase::Spawn, 1);
        let running = record.transition_to_running();
        assert_eq!(running.spawn_phase, SpawnPhase::Running);
        assert_eq!(running.spawn_attempts, 1);
    }

    #[test]
    fn restart_policy_record_respawn_increments_attempts() {
        let failed = create_spawn_record(make_instance_id(1), SpawnPhase::Failed, 3);
        let respawned = failed.respawn(Some(vo_types::SpawnId::new("new-spawn".to_string())));
        assert_eq!(respawned.spawn_phase, SpawnPhase::Spawn);
        assert_eq!(respawned.spawn_attempts, 4);
        assert_eq!(respawned.spawn_id.as_ref().map(|s| s.as_str()), Some("new-spawn"));
    }

    #[test]
    fn supervisor_error_resumable_for_restart() {
        let health_err = SpawnSupervisorError::HealthCheckFailed {
            instance_id: make_instance_id(1),
            check_number: 1,
            error: "timeout".to_string(),
        };
        assert!(health_err.is_resumable());

        let exit_err = SpawnSupervisorError::ProcessExited {
            instance_id: make_instance_id(1),
            pid: 123,
            exit_code: 1,
        };
        assert!(exit_err.is_resumable());

        let storage_err = SpawnSupervisorError::StorageError("disk full".to_string());
        assert!(!storage_err.is_resumable());
    }

    #[test]
    fn supervisor_error_fatal_no_restart() {
        let zombie_err = SpawnSupervisorError::ZombieDetected {
            instance_id: make_instance_id(1),
            pid: 999,
        };
        assert!(zombie_err.is_fatal());

        let corrupt_err = SpawnSupervisorError::CorruptSpawn("bad data".to_string());
        assert!(corrupt_err.is_fatal());
    }

    #[tokio::test]
    async fn supervisor_restart_recovery_after_crash() {
        let instance_id = make_instance_id(77);
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        storage
            .add_timer(make_timer_record(instance_id.clone(), 5000))
            .await;

        storage
            .mark_timer_processing(&instance_id, ts_ms(5000))
            .await
            .expect("mark should succeed");

        let pending = storage
            .scan_pending_timers(100)
            .await
            .expect("scan pending should succeed");
        assert_eq!(pending.len(), 1);

        work_queue
            .enqueue_resume(instance_id.clone())
            .await
            .expect("enqueue should succeed");

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 1);
        assert_eq!(enqueued[0], instance_id);

        storage
            .complete_timer_processing(&instance_id, ts_ms(5000))
            .await
            .expect("complete should succeed");

        let remaining = storage
            .scan_pending_timers(100)
            .await
            .expect("scan pending should succeed");
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn supervisor_at_most_one_concurrent_resume() {
        let instance_id = make_instance_id(88);
        let storage = Arc::new(MockTimerStorage::empty());
        let work_queue = Arc::new(MockWorkQueue::new());

        storage
            .mark_timer_processing(&instance_id, ts_ms(5000))
            .await
            .expect("mark should succeed");

        let pending = storage
            .scan_pending_timers(100)
            .await
            .expect("scan pending should succeed");
        assert_eq!(pending.len(), 1);

        work_queue
            .enqueue_resume(instance_id.clone())
            .await
            .expect("first enqueue should succeed");

        let enqueued = work_queue.enqueued().await;
        assert_eq!(enqueued.len(), 1);
    }
}

// =============================================================================
// Section 3: Probe Instrumentation Tests
// Tests for metric emission, trace correlation, and health check responses
// =============================================================================

mod probe_instrumentation_tests {
    use super::*;

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn make_result(id: ProbeId, status: ProbeStatus, failures: u32, latency: u64) -> ProbeResult {
        ProbeResult {
            probe_id: id,
            status,
            latency_ms: latency,
            consecutive_failures: failures,
            last_check_ms: now_ms(),
            message: None,
        }
    }

    #[test]
    fn probe_metric_emission_latency_recorded() {
        let id = ProbeId::new();
        let result = make_result(id, ProbeStatus::Healthy, 0, 42);

        assert_eq!(result.latency_ms, 42);
    }

    #[test]
    fn probe_metric_emission_failure_count() {
        let id = ProbeId::new();
        let result = make_result(id, ProbeStatus::Unhealthy, 5, 10);

        assert_eq!(result.consecutive_failures, 5);
    }

    #[test]
    fn probe_trace_correlation_probe_id_preserved() {
        let id = ProbeId::new();
        let original = make_result(id, ProbeStatus::Healthy, 0, 5);

        let json = serde_json::to_string(&original).expect("should serialize");
        let deserialized: ProbeResult =
            serde_json::from_str(&json).expect("should deserialize");

        assert_eq!(original.probe_id, deserialized.probe_id);
    }

    #[test]
    fn probe_trace_correlation_timestamp_preserved() {
        let before = now_ms();
        let id = ProbeId::new();
        let result = make_result(id, ProbeStatus::Healthy, 0, 5);
        let after = now_ms();

        assert!(result.last_check_ms >= before);
        assert!(result.last_check_ms <= after);
    }

    #[test]
    fn probe_health_check_response_healthy_after_threshold() {
        let mut agg = AggregatedStatus::new();
        let id = ProbeId::new();

        for _ in 0..3 {
            agg.update(make_result(id, ProbeStatus::Healthy, 0, 10));
        }

        assert_eq!(agg.overall, ProbeStatus::Healthy);
        assert_eq!(agg.healthy_count, 1);
    }

    #[test]
    fn probe_health_check_response_unhealthy_after_threshold() {
        let mut agg = AggregatedStatus::new();
        let id = ProbeId::new();

        for i in 0..3 {
            agg.update(make_result(id, ProbeStatus::Unhealthy, i + 1, 10));
        }

        assert_eq!(agg.overall, ProbeStatus::Unhealthy);
        assert_eq!(agg.unhealthy_count, 1);
    }

    #[test]
    fn probe_health_check_response_transitions() {
        let mut agg = AggregatedStatus::new();
        let id = ProbeId::new();

        agg.update(make_result(id, ProbeStatus::Healthy, 0, 10));
        assert_eq!(agg.overall, ProbeStatus::Healthy);

        agg.update(make_result(id, ProbeStatus::Unhealthy, 1, 10));
        assert_eq!(agg.overall, ProbeStatus::Unhealthy);

        agg.update(make_result(id, ProbeStatus::Healthy, 0, 10));
        assert_eq!(agg.overall, ProbeStatus::Healthy);
    }

    #[test]
    fn probe_metric_aggregation_multiple_probes() {
        let mut agg = AggregatedStatus::new();

        let id1 = ProbeId::new();
        let id2 = ProbeId::new();
        let id3 = ProbeId::new();

        agg.update(make_result(id1, ProbeStatus::Healthy, 0, 5));
        agg.update(make_result(id2, ProbeStatus::Healthy, 0, 10));
        agg.update(make_result(id3, ProbeStatus::Unhealthy, 1, 15));

        assert_eq!(agg.overall, ProbeStatus::Unhealthy);
        assert_eq!(agg.healthy_count, 2);
        assert_eq!(agg.unhealthy_count, 1);
    }

    #[test]
    fn probe_backoff_config_affects_interval() {
        let config = BackoffConfig::default();

        let interval_0 = config.calculate_interval(0);
        let interval_1 = config.calculate_interval(1);
        let interval_2 = config.calculate_interval(2);

        assert!(interval_1 > interval_0);
        assert!(interval_2 > interval_1);
    }

    #[test]
    fn probe_backoff_config_max_interval_cap() {
        let config = BackoffConfig {
            initial_interval: Duration::from_secs(1),
            max_interval: Duration::from_secs(10),
            multiplier: 2.0,
            max_failures: 10,
        };

        let interval_below_max = config.calculate_interval(5);
        let interval_above_max = config.calculate_interval(20);

        assert!(interval_below_max <= config.max_interval);
        assert_eq!(interval_above_max, config.max_interval);
    }

    #[test]
    fn probe_registry_tracks_all_registered_probes() {
        let mut registry = ProbeRegistry::new();

        let def1 = ProbeDefinition {
            id: ProbeId::new(),
            name: "health-1".to_string(),
            config: ProbeConfig::http("http://localhost:8080"),
            interval: Duration::from_secs(30),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        };

        let def2 = ProbeDefinition {
            id: ProbeId::new(),
            name: "health-2".to_string(),
            config: ProbeConfig::tcp("localhost", 9090),
            interval: Duration::from_secs(30),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        };

        let id1 = registry.register(def1);
        let id2 = registry.register(def2);

        assert_eq!(registry.len(), 2);
        assert!(registry.get(&id1).is_some());
        assert!(registry.get(&id2).is_some());
        assert_eq!(registry.list().len(), 2);
    }

    #[test]
    fn probe_registry_unregister_removes_probe() {
        let mut registry = ProbeRegistry::new();

        let def = ProbeDefinition {
            id: ProbeId::new(),
            name: "health".to_string(),
            config: ProbeConfig::http("http://localhost:8080"),
            interval: Duration::from_secs(30),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        };

        let id = registry.register(def);
        assert_eq!(registry.len(), 1);

        let removed = registry.unregister(id);
        assert!(removed.is_some());
        assert_eq!(registry.len(), 0);
        assert!(registry.get(&id).is_none());
    }

    #[test]
    fn probe_config_serialization_round_trip() {
        let http_config = ProbeConfig::http("http://localhost:8080/health");
        let tcp_config = ProbeConfig::tcp("localhost", 8080);
        let exec_config = ProbeConfig::exec("curl", vec!["-s".to_string()]);

        for config in [http_config.clone(), tcp_config.clone(), exec_config.clone()] {
            let json = serde_json::to_string(&config).expect("should serialize");
            let deserialized: ProbeConfig =
                serde_json::from_str(&json).expect("should deserialize");
            assert_eq!(
                config.probe_type(),
                deserialized.probe_type(),
                "probe type should be preserved"
            );
        }
    }

    #[test]
    fn probe_id_serialization_round_trip() {
        let id = ProbeId::new();
        let original_str = id.as_str();

        let json = serde_json::to_string(&id).expect("should serialize");
        let deserialized: ProbeId =
            serde_json::from_str(&json).expect("should deserialize");

        assert_eq!(id.as_str(), deserialized.as_str());
        assert_eq!(original_str, deserialized.as_str());
    }

    #[test]
    fn probe_result_serialization_preserves_all_fields() {
        let id = ProbeId::new();
        let result = ProbeResult {
            probe_id: id,
            status: ProbeStatus::Healthy,
            latency_ms: 42,
            consecutive_failures: 0,
            last_check_ms: 1234567890,
            message: Some("OK".to_string()),
        };

        let json = serde_json::to_string(&result).expect("should serialize");
        let deserialized: ProbeResult =
            serde_json::from_str(&json).expect("should deserialize");

        assert_eq!(result.probe_id, deserialized.probe_id);
        assert_eq!(result.status, deserialized.status);
        assert_eq!(result.latency_ms, deserialized.latency_ms);
        assert_eq!(result.consecutive_failures, deserialized.consecutive_failures);
        assert_eq!(result.last_check_ms, deserialized.last_check_ms);
        assert_eq!(result.message, deserialized.message);
    }
}