//! Component integration tests for vo-core.
//!
//! These tests exercise integration between multiple vo-core components to verify
//! that they work correctly when composed together.
//!
//! Components tested:
//! - WriteClass (write taxonomy)
//! - ResourceQuota (resource limits)
//! - Admission (write pressure handling)
//! - CircuitBreaker (fault tolerance)
//! - Replay (event sourcing)
//! - Upcaster (event versioning)

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, CircuitBreakerConfig, CircuitBreakerState,
    RegistrationOutcome, RegistrationRequest, RegistrationStatus,
};
use vo_core::resource_quota::{
    NamespaceQuota, NamespaceRegistry, OvercommitPolicy, QuotaEnforcer, QuotaUsage,
};
use vo_core::write_class::{WriteBudget, WriteClass};

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn make_wf(s: &str) -> vo_types::WorkflowName {
    vo_types::WorkflowName::parse(s).expect("test workflow name should be valid")
}

fn make_hash(s: &str) -> vo_types::BinaryHash {
    vo_types::BinaryHash::parse(s).expect("test hash should be valid")
}

fn default_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5)
        .expect("default config should be valid")
}

fn make_request(wf: &str, hash: &str, force: bool) -> RegistrationRequest {
    RegistrationRequest {
        workflow_name: make_wf(wf),
        binary_hash: make_hash(hash),
        force,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-01: WriteClass + ResourceQuota Integration
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn write_class_and_resource_quota_compose_correctly() {
    let mut enforcer = QuotaEnforcer::with_default_namespace();
    let quota = NamespaceQuota::new("test-ns")
        .with_cpu(vo_core::resource_quota::CpuQuota::new(
            std::num::NonZeroU64::new(4).expect("non-zero"),
        ))
        .with_memory(vo_core::resource_quota::MemoryQuota::new(
            std::num::NonZeroU64::new(1024).expect("non-zero"),
        ))
        .with_overcommit(OvercommitPolicy::AllowOvercommit);

    enforcer
        .registry_mut()
        .register(quota)
        .expect("quota registration should succeed");

    let result = enforcer.check_cpu("test-ns", 2);
    assert!(result.is_ok(), "cpu check within quota should succeed");

    let result = enforcer.check_memory("test-ns", 512);
    assert!(result.is_ok(), "memory check within quota should succeed");

    let budget = WriteBudget::new(100, 200, 300);
    assert!(budget.can_write(WriteClass::CriticalControlPlane, 50));
    assert!(budget.reserve(WriteClass::CriticalControlPlane, 50).is_ok());
    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        50,
        "remaining budget should be 50 after reserve"
    );
}

#[test]
fn write_class_tier_influence_resource_allocation() {
    let budget = WriteBudget::new(100, 200, 300);

    let critical_reserve = budget.reserve(WriteClass::CriticalControlPlane, 100);
    assert!(
        critical_reserve.is_ok(),
        "critical tier should allow full allocation"
    );

    let projection_reserve = budget.reserve(WriteClass::OperatorProjection, 200);
    assert!(
        projection_reserve.is_ok(),
        "projection tier should allow full allocation"
    );

    let bulk_reserve = budget.reserve(WriteClass::BulkBlob, 300);
    assert!(
        bulk_reserve.is_ok(),
        "bulk tier should allow full allocation"
    );

    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        0,
        "critical tier should be exhausted"
    );
    assert_eq!(
        budget.remaining(WriteClass::OperatorProjection),
        0,
        "projection tier should be exhausted"
    );
    assert_eq!(
        budget.remaining(WriteClass::BulkBlob),
        0,
        "bulk tier should be exhausted"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-02: CircuitBreaker + Workflow Registration Integration
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn circuit_breaker_and_workflow_registration_compose() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    let request = make_request("deploy-prod", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::Allowed),
        "first registration should be allowed"
    );

    let failure_result = record_failure(
        &make_wf("deploy-prod"),
        &make_hash("aaaa0001"),
        &config,
        &state,
        now,
    );
    assert_eq!(
        failure_result,
        Ok(None),
        "first failure should not trigger quarantine"
    );

    for i in 2..6 {
        let hash = format!("aaaa000{}", i);
        let result = record_failure(
            &make_wf("deploy-prod"),
            &make_hash(&hash),
            &config,
            &state,
            now,
        );
        if i < 5 {
            assert_eq!(
                result,
                Ok(None),
                "failure {} should not trigger quarantine",
                i
            );
        } else {
            assert!(
                result.is_ok(),
                "5th unique failure should return Ok (quarantine event or success)"
            );
        }
    }
}

#[test]
fn circuit_breaker_workflow_isolation_across_namespaces() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    for i in 0..5 {
        let hash = format!("aaaa000{}", i);
        record_failure(
            &make_wf("workflow-a"),
            &make_hash(&hash),
            &config,
            &state,
            now,
        )
        .expect("failure recording should succeed");
    }

    let status_a = state.get_status(&make_wf("workflow-a"));
    assert_eq!(
        status_a,
        RegistrationStatus::Quarantined,
        "workflow-a should be quarantined"
    );

    let status_b = state.get_status(&make_wf("workflow-b"));
    assert_eq!(
        status_b,
        RegistrationStatus::Active,
        "workflow-b should remain active (isolation)"
    );

    let request = make_request("workflow-b", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::Allowed),
        "workflow-b should be allowed"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-03: ResourceQuota + Admission Integration
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn resource_quota_usage_tracking() {
    let mut enforcer = QuotaEnforcer::with_default_namespace();

    let quota = NamespaceQuota::new("payments")
        .with_cpu(vo_core::resource_quota::CpuQuota::new(
            std::num::NonZeroU64::new(8).expect("non-zero"),
        ))
        .with_memory(vo_core::resource_quota::MemoryQuota::new(
            std::num::NonZeroU64::new(4096).expect("non-zero"),
        ))
        .with_disk(vo_core::resource_quota::DiskQuota::new(
            std::num::NonZeroU64::new(10240).expect("non-zero"),
        ));

    enforcer
        .registry_mut()
        .register(quota)
        .expect("quota registration should succeed");

    let usage = QuotaUsage::new()
        .with_cpu(4)
        .with_memory(2048)
        .with_disk(5120);

    let cpu_result = enforcer.check_cpu("payments", usage.cpu_cores_used);
    assert!(cpu_result.is_ok(), "cpu usage within quota should pass");

    let mem_result = enforcer.check_memory("payments", usage.memory_bytes_used);
    assert!(mem_result.is_ok(), "memory usage within quota should pass");

    let disk_result = enforcer.check_disk("payments", usage.disk_bytes_used);
    assert!(disk_result.is_ok(), "disk usage within quota should pass");

    let over_limit_usage = QuotaUsage::new().with_cpu(16).with_memory(8192);

    let cpu_over = enforcer.check_cpu("payments", over_limit_usage.cpu_cores_used);
    assert!(cpu_over.is_err(), "cpu usage over quota should fail");

    let mem_over = enforcer.check_memory("payments", over_limit_usage.memory_bytes_used);
    assert!(mem_over.is_err(), "memory usage over quota should fail");
}

#[test]
fn quota_enforcer_default_namespace_works() {
    let enforcer = QuotaEnforcer::with_default_namespace();

    let result = enforcer.check_cpu("default", 0);
    assert!(result.is_ok(), "zero cpu check should always pass");

    let result = enforcer.check_memory("default", u64::MAX);
    assert!(result.is_err(), "max memory should exceed default quota");
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-04: WriteBudget Multi-Tier Budget Isolation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn write_budget_tier_isolation() {
    let budget = WriteBudget::new(100, 200, 300);

    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        100,
        "critical tier starts with full budget"
    );
    assert_eq!(
        budget.remaining(WriteClass::OperatorProjection),
        200,
        "projection tier starts with full budget"
    );
    assert_eq!(
        budget.remaining(WriteClass::BulkBlob),
        300,
        "bulk tier starts with full budget"
    );

    budget
        .reserve(WriteClass::CriticalControlPlane, 50)
        .expect("reserve should succeed");
    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        50,
        "critical tier should have 50 remaining"
    );

    assert_eq!(
        budget.remaining(WriteClass::OperatorProjection),
        200,
        "projection tier should be unaffected"
    );
    assert_eq!(
        budget.remaining(WriteClass::BulkBlob),
        300,
        "bulk tier should be unaffected"
    );
}

#[test]
fn write_budget_critical_never_drops_flag() {
    let critical = WriteClass::CriticalControlPlane;
    let projection = WriteClass::OperatorProjection;
    let bulk = WriteClass::BulkBlob;

    assert!(
        critical.never_drops(),
        "critical control plane writes should never be dropped"
    );
    assert!(
        !projection.never_drops(),
        "operator projection writes may be dropped under pressure"
    );
    assert!(
        !bulk.never_drops(),
        "bulk blob writes may be deferred under pressure"
    );
}

#[test]
fn write_budget_can_write_consistency() {
    let budget = WriteBudget::new(100, 200, 300);

    let test_cases = [
        (WriteClass::CriticalControlPlane, 0u64),
        (WriteClass::CriticalControlPlane, 50),
        (WriteClass::CriticalControlPlane, 100),
        (WriteClass::OperatorProjection, 150),
        (WriteClass::BulkBlob, 250),
    ];

    for (class, size) in test_cases {
        let can_write = budget.can_write(class, size);
        let reserve_result = budget.reserve(class, size);
        assert_eq!(
            can_write,
            reserve_result.is_ok(),
            "can_write({:?}, {}) should match reserve result",
            class,
            size
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-05: CircuitBreaker Error Handling Integration
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn circuit_breaker_rate_limit_enforces_cooldown() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    let request = make_request("deploy-prod", "abcdef01", false);
    let result1 = evaluate_registration(&request, &config, &state, t0);
    assert_eq!(
        result1,
        Ok(RegistrationOutcome::Allowed),
        "first registration should succeed"
    );

    let t_within_limit = t0 + Duration::from_secs(30);
    let result2 = evaluate_registration(&request, &config, &state, t_within_limit);
    assert_eq!(
        result2,
        Ok(RegistrationOutcome::RateLimited {
            retry_after_secs: 30
        }),
        "second registration within 60s window should be rate-limited"
    );

    let t_past_limit = t0 + Duration::from_secs(120);
    let result3 = evaluate_registration(&request, &config, &state, t_past_limit);
    assert_eq!(
        result3,
        Ok(RegistrationOutcome::Allowed),
        "registration after rate limit window should succeed"
    );
}

#[test]
fn circuit_breaker_force_bypasses_all_protections() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    state
        .statuses
        .insert(make_wf("stuck-wf"), RegistrationStatus::Quarantined);
    state
        .rate_limiter
        .insert(make_wf("stuck-wf"), now - Duration::from_secs(30));

    let force_request = make_request("stuck-wf", "abcdef01", true);
    let result = evaluate_registration(&force_request, &config, &state, now);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::Allowed),
        "force registration should bypass quarantine"
    );

    let status = state.get_status(&make_wf("stuck-wf"));
    assert_eq!(
        status,
        RegistrationStatus::Quarantined,
        "force should not change quarantine status"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-06: NamespaceQuota Per-Namespace Isolation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn namespace_quota_isolation_between_namespaces() {
    let mut enforcer = QuotaEnforcer::with_default_namespace();

    let payments_quota = NamespaceQuota::new("payments")
        .with_cpu(vo_core::resource_quota::CpuQuota::new(
            std::num::NonZeroU64::new(2).expect("non-zero"),
        ))
        .with_overcommit(OvercommitPolicy::AllowOvercommit);

    let analytics_quota = NamespaceQuota::new("analytics")
        .with_cpu(vo_core::resource_quota::CpuQuota::new(
            std::num::NonZeroU64::new(8).expect("non-zero"),
        ))
        .with_overcommit(OvercommitPolicy::AllowOvercommit);

    enforcer
        .registry_mut()
        .register(payments_quota)
        .expect("payments quota registration should succeed");
    enforcer
        .registry_mut()
        .register(analytics_quota)
        .expect("analytics quota registration should succeed");

    let payments_result = enforcer.check_cpu("payments", 2);
    assert!(payments_result.is_ok(), "payments at limit should pass");

    let analytics_result = enforcer.check_cpu("analytics", 4);
    assert!(
        analytics_result.is_ok(),
        "analytics well under limit should pass"
    );

    let payments_over = enforcer.check_cpu("payments", 3);
    assert!(payments_over.is_err(), "payments over limit should fail");

    let unknown_result = enforcer.check_cpu("unknown-ns", 1);
    assert!(
        unknown_result.is_ok(),
        "unknown namespace should use default (pass for small values)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-07: WriteClass Serialization Round-Trip
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn write_class_json_serialization_roundtrip() {
    let classes = [
        WriteClass::CriticalControlPlane,
        WriteClass::OperatorProjection,
        WriteClass::BulkBlob,
    ];

    for class in classes {
        let json = serde_json::to_string(&class).expect("serialization should succeed");
        let parsed: WriteClass =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(
            parsed, class,
            "WriteClass {:?} should round-trip through JSON",
            class
        );
    }
}

#[test]
fn write_class_parse_and_as_str_consistency() {
    let classes = [
        (WriteClass::CriticalControlPlane, "critical_control_plane"),
        (WriteClass::OperatorProjection, "operator_projection"),
        (WriteClass::BulkBlob, "bulk_blob"),
    ];

    for (class, expected_str) in classes {
        assert_eq!(
            class.as_str(),
            expected_str,
            "as_str() should return canonical string"
        );
        let parsed = WriteClass::parse(expected_str).expect("parse should succeed");
        assert_eq!(
            parsed, class,
            "parse(\"{}\") should return {:?}",
            expected_str, class
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-08: ResourceKind Serialization
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn resource_kind_serialization_roundtrip() {
    use vo_core::resource_quota::ResourceKind;

    let kinds = [ResourceKind::Cpu, ResourceKind::Memory, ResourceKind::Disk];

    for kind in kinds {
        let json = serde_json::to_string(&kind).expect("serialization should succeed");
        let parsed: ResourceKind =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(
            parsed, kind,
            "ResourceKind {:?} should round-trip through JSON",
            kind
        );
    }

    assert_eq!(ResourceKind::Cpu.as_str(), "cpu");
    assert_eq!(ResourceKind::Memory.as_str(), "memory");
    assert_eq!(ResourceKind::Disk.as_str(), "disk");
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-09: CircuitBreaker Failure Window Tracking
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn circuit_breaker_failure_window_respects_time() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    record_failure(
        &make_wf("decay-wf"),
        &make_hash("aaaa0001"),
        &config,
        &state,
        t0,
    )
    .expect("first failure should be recorded");
    record_failure(
        &make_wf("decay-wf"),
        &make_hash("aaaa0002"),
        &config,
        &state,
        t0,
    )
    .expect("second failure should be recorded");
    record_failure(
        &make_wf("decay-wf"),
        &make_hash("aaaa0003"),
        &config,
        &state,
        t0,
    )
    .expect("third failure should be recorded");
    record_failure(
        &make_wf("decay-wf"),
        &make_hash("aaaa0004"),
        &config,
        &state,
        t0,
    )
    .expect("fourth failure should be recorded");

    let status_before = state.get_status(&make_wf("decay-wf"));
    assert_eq!(
        status_before,
        RegistrationStatus::Active,
        "4 failures should not trigger quarantine (threshold 5)"
    );

    record_failure(
        &make_wf("decay-wf"),
        &make_hash("aaaa0005"),
        &config,
        &state,
        t0,
    )
    .expect("fifth failure should be recorded");

    let status_after = state.get_status(&make_wf("decay-wf"));
    assert_eq!(
        status_after,
        RegistrationStatus::Quarantined,
        "5th unique failure should trigger quarantine"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-10: OvercommitPolicy Behavior
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn overcommit_policy_default_is_no_overcommit() {
    let policy = OvercommitPolicy::default();
    assert_eq!(
        policy,
        OvercommitPolicy::NoOvercommit,
        "default overcommit policy should be NoOvercommit"
    );
}

#[test]
fn overcommit_policy_variants() {
    let policies = [
        OvercommitPolicy::NoOvercommit,
        OvercommitPolicy::AllowOvercommit,
    ];

    assert_eq!(
        policies.len(),
        2,
        "should have exactly 2 overcommit policy variants"
    );

    for policy in policies {
        let json = serde_json::to_string(&policy).expect("serialization should succeed");
        let parsed: OvercommitPolicy =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(parsed, policy, "OvercommitPolicy should round-trip");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-11: QuotaUsage Construction
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn quota_usage_builder_pattern() {
    let usage = QuotaUsage::new()
        .with_cpu(4)
        .with_memory(2048)
        .with_disk(4096);

    assert_eq!(usage.cpu_cores_used, 4);
    assert_eq!(usage.memory_bytes_used, 2048);
    assert_eq!(usage.disk_bytes_used, 4096);
}

#[test]
fn quota_usage_default_is_zero() {
    let usage = QuotaUsage::new();
    assert_eq!(usage.cpu_cores_used, 0);
    assert_eq!(usage.memory_bytes_used, 0);
    assert_eq!(usage.disk_bytes_used, 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-12: NamespaceRegistry Registration
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn namespace_registry_registration_and_lookup() {
    use vo_core::resource_quota::NamespaceRegistry;

    let mut registry = NamespaceRegistry::new();

    let quota = NamespaceQuota::new("test-ns").with_cpu(vo_core::resource_quota::CpuQuota::new(
        std::num::NonZeroU64::new(4).expect("non-zero"),
    ));

    let result = registry.register(quota.clone());
    assert!(result.is_ok(), "quota registration should succeed");

    let looked_up = registry.get("test-ns");
    assert!(
        looked_up.is_some(),
        "registered namespace should be findable"
    );
    assert_eq!(
        looked_up.map(|q| q.namespace.as_str()),
        Some("test-ns"),
        "namespace name should match"
    );

    let not_found = registry.get("unknown-ns");
    assert!(not_found.is_none(), "unknown namespace should not be found");
}

#[test]
fn namespace_registry_duplicate_registration_fails() {
    use vo_core::resource_quota::NamespaceRegistry;

    let mut registry = NamespaceRegistry::new();

    let quota = NamespaceQuota::new("dup-ns").with_cpu(vo_core::resource_quota::CpuQuota::new(
        std::num::NonZeroU64::new(4).expect("non-zero"),
    ));

    let result1 = registry.register(quota.clone());
    assert!(result1.is_ok(), "first registration should succeed");

    let result2 = registry.register(quota);
    assert!(result2.is_err(), "duplicate registration should fail");
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-13: CircuitBreaker State Accessors
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn circuit_breaker_state_get_status_unknown_workflow() {
    let state = CircuitBreakerState::new();
    let status = state.get_status(&make_wf("unknown-wf"));
    assert_eq!(
        status,
        RegistrationStatus::Active,
        "unknown workflow should default to Active"
    );
}

#[test]
fn circuit_breaker_state_set_status() {
    let state = CircuitBreakerState::new();

    state.set_status(make_wf("test-wf"), RegistrationStatus::Quarantined);

    let status = state.get_status(&make_wf("test-wf"));
    assert_eq!(
        status,
        RegistrationStatus::Quarantined,
        "status should be Quarantined after set_status"
    );
}

#[test]
fn circuit_breaker_state_clear_status() {
    let state = CircuitBreakerState::new();

    state.set_status(make_wf("test-wf"), RegistrationStatus::Quarantined);
    assert_eq!(
        state.get_status(&make_wf("test-wf")),
        RegistrationStatus::Quarantined
    );

    state.set_status(make_wf("test-wf"), RegistrationStatus::Active);
    assert_eq!(
        state.get_status(&make_wf("test-wf")),
        RegistrationStatus::Active,
        "status should be Active after set_status to Active"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// CI-14: WritePressureMetrics Collection and Reporting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn write_pressure_metrics_gauge_thread_safety() {
    use std::sync::Arc;
    use std::thread;
    use vo_core::admission::metrics::Gauge;

    let gauge = Arc::new(Gauge::new());
    let gauge_clone = gauge.clone();

    let handle = thread::spawn(move || {
        gauge_clone.set(42);
    });

    handle.join().expect("thread should complete");

    assert_eq!(
        gauge.get(),
        42,
        "gauge should reflect value set by other thread"
    );
}

#[test]
fn write_pressure_metrics_bool_gauge_thread_safety() {
    use std::sync::Arc;
    use std::thread;
    use vo_core::admission::metrics::BoolGauge;

    let bool_gauge = Arc::new(BoolGauge::new());
    let bool_gauge_clone = bool_gauge.clone();

    let handle = thread::spawn(move || {
        bool_gauge_clone.set(true);
    });

    handle.join().expect("thread should complete");

    assert!(
        bool_gauge.get(),
        "bool gauge should reflect value set by other thread"
    );
}

#[test]
fn write_pressure_metrics_update_from_admission_state() {
    use vo_core::admission::metrics::WritePressureMetrics;
    use vo_core::admission::types::WritePressureState;

    let metrics = WritePressureMetrics::new();

    let high_pressure_state = WritePressureState {
        writer_queue_depth: 1000,
        batch_commit_latency_ms: 5000,
        blob_queue_depth: 500,
        compaction_stall_active: true,
        storage_stall_active: true,
    };

    metrics.update_from_state(&high_pressure_state);

    assert_eq!(
        metrics.writer_queue_depth.get(),
        1000,
        "writer queue depth should reflect high pressure"
    );
    assert_eq!(
        metrics.batch_commit_latency_ms.get(),
        5000,
        "batch commit latency should reflect high pressure"
    );
    assert_eq!(
        metrics.blob_queue_depth.get(),
        500,
        "blob queue depth should reflect high pressure"
    );
    assert!(
        metrics.compaction_stall_active.get(),
        "compaction stall should be active"
    );
    assert!(
        metrics.storage_stall_active.get(),
        "storage stall should be active"
    );
}

#[test]
fn write_pressure_metrics_zero_state() {
    use vo_core::admission::metrics::WritePressureMetrics;
    use vo_core::admission::types::WritePressureState;

    let metrics = WritePressureMetrics::new();

    let zero_state = WritePressureState::default();
    metrics.update_from_state(&zero_state);

    assert_eq!(
        metrics.writer_queue_depth.get(),
        0,
        "writer queue depth should be zero"
    );
    assert_eq!(
        metrics.batch_commit_latency_ms.get(),
        0,
        "batch commit latency should be zero"
    );
    assert_eq!(
        metrics.blob_queue_depth.get(),
        0,
        "blob queue depth should be zero"
    );
    assert!(
        !metrics.compaction_stall_active.get(),
        "compaction stall should be inactive"
    );
    assert!(
        !metrics.storage_stall_active.get(),
        "storage stall should be inactive"
    );
}

#[test]
fn write_pressure_metrics_reported_values_match_state() {
    use vo_core::admission::metrics::WritePressureMetrics;
    use vo_core::admission::types::WritePressureState;

    let metrics = WritePressureMetrics::new();

    let test_cases = [
        WritePressureState {
            writer_queue_depth: 0,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: false,
            storage_stall_active: false,
        },
        WritePressureState {
            writer_queue_depth: 50,
            batch_commit_latency_ms: 100,
            blob_queue_depth: 25,
            compaction_stall_active: false,
            storage_stall_active: false,
        },
        WritePressureState {
            writer_queue_depth: u64::MAX,
            batch_commit_latency_ms: u64::MAX,
            blob_queue_depth: u64::MAX,
            compaction_stall_active: true,
            storage_stall_active: true,
        },
    ];

    for state in test_cases {
        metrics.update_from_state(&state);

        assert_eq!(
            metrics.writer_queue_depth.get(),
            state.writer_queue_depth,
            "gauge should match state"
        );
        assert_eq!(
            metrics.batch_commit_latency_ms.get(),
            state.batch_commit_latency_ms,
            "gauge should match state"
        );
        assert_eq!(
            metrics.blob_queue_depth.get(),
            state.blob_queue_depth,
            "gauge should match state"
        );
        assert_eq!(
            metrics.compaction_stall_active.get(),
            state.compaction_stall_active,
            "gauge should match state"
        );
        assert_eq!(
            metrics.storage_stall_active.get(),
            state.storage_stall_active,
            "gauge should match state"
        );
    }
}
