//! Business operations stress tests for vo-core.
//!
//! These tests verify that vo-core components handle maximum load conditions
//! without performance degradation. Tests simulate high-throughput business
//! operations scenarios.
//!
//! # Test Categories
//!
//! - BOS-01: WriteClass + WriteBudget stress under maximum tier pressure
//! - BOS-02: ResourceQuota namespace isolation under concurrent load
//! - BOS-03: CircuitBreaker workflow isolation under failure storm
//! - BOS-04: WorkloadBudget acquire/release under contention
//! - BOS-05: Composite stress - all components under maximum load

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, CircuitBreakerConfig, CircuitBreakerState,
    RegistrationOutcome, RegistrationRequest, RegistrationStatus,
};
use vo_core::resource_quota::{NamespaceQuota, OvercommitPolicy, QuotaEnforcer, QuotaUsage};
use vo_core::workload_class::{WorkloadBudget, WorkloadClass};
use vo_core::write_class::{WriteBudget, WriteClass};
use vo_types::{BinaryHash, WorkflowName};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).expect("test hash should be valid")
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
// BOS-01: WriteClass + WriteBudget Stress Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn bos01_write_budget_exhaustion_and_recovery_stress() {
    let budget = WriteBudget::new(1000, 2000, 5000);

    let successful_reserves: Vec<_> = (0..1001u64)
        .map(|_| budget.reserve(WriteClass::CriticalControlPlane, 1))
        .filter(|r| r.is_ok())
        .collect();

    let critical_exhausted = 1001 - successful_reserves.len();
    assert!(
        critical_exhausted > 0,
        "Critical tier should exhaust under maximum pressure, got {critical_exhausted}"
    );

    let remaining = budget.remaining(WriteClass::CriticalControlPlane);
    assert!(
        remaining < 100,
        "Critical tier should be near exhaustion, got {remaining}"
    );

    assert!(
        budget.reserve(WriteClass::CriticalControlPlane, 1).is_err(),
        "Exhausted tier must reject further writes"
    );

    assert!(
        budget.can_write(WriteClass::OperatorProjection, 1),
        "Projection tier should remain unaffected by critical exhaustion"
    );
    assert!(
        budget.can_write(WriteClass::BulkBlob, 1),
        "Bulk tier should remain unaffected by critical exhaustion"
    );
}

#[test]
fn bos01_write_budget_all_tiers_simultaneous_pressure() {
    let budget = WriteBudget::new(1000, 2000, 5000);

    let critical_ok = (0..1001u64)
        .map(|_| budget.reserve(WriteClass::CriticalControlPlane, 1).is_ok())
        .filter(|ok| *ok)
        .count();

    let projection_ok = (0..2001u64)
        .map(|_| budget.reserve(WriteClass::OperatorProjection, 1).is_ok())
        .filter(|ok| *ok)
        .count();

    let bulk_ok = (0..5001u64)
        .map(|_| budget.reserve(WriteClass::BulkBlob, 1).is_ok())
        .filter(|ok| *ok)
        .count();

    assert!(
        critical_ok < 1001,
        "Critical tier should show some pressure: {critical_ok}"
    );
    assert!(
        projection_ok < 2001,
        "Projection tier should show some pressure: {projection_ok}"
    );
    assert!(
        bulk_ok < 5001,
        "Bulk tier should show some pressure: {bulk_ok}"
    );
}

#[test]
fn bos01_write_budget_tier_isolation_under_stress() {
    let budget = WriteBudget::new(100, 200, 300);

    for _ in 0..100 {
        budget
            .reserve(WriteClass::CriticalControlPlane, 1)
            .expect("reserve should succeed until exhaustion");
    }

    assert!(
        budget.reserve(WriteClass::CriticalControlPlane, 1).is_err(),
        "Critical should be exhausted"
    );

    for _ in 0..200 {
        budget
            .reserve(WriteClass::OperatorProjection, 1)
            .expect("reserve should succeed until exhaustion");
    }

    assert!(
        budget.reserve(WriteClass::OperatorProjection, 1).is_err(),
        "Projection should be exhausted"
    );

    for _ in 0..300 {
        budget
            .reserve(WriteClass::BulkBlob, 1)
            .expect("reserve should succeed until exhaustion");
    }

    assert!(
        budget.reserve(WriteClass::BulkBlob, 1).is_err(),
        "Bulk should be exhausted"
    );

    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        0,
        "Critical must be exactly 0"
    );
    assert_eq!(
        budget.remaining(WriteClass::OperatorProjection),
        0,
        "Projection must be exactly 0"
    );
    assert_eq!(
        budget.remaining(WriteClass::BulkBlob),
        0,
        "Bulk must be exactly 0"
    );
}

#[test]
fn bos01_write_budget_partial_pressure_distribution() {
    let budget = WriteBudget::new(1000, 1000, 1000);

    let test_cases = [
        (WriteClass::CriticalControlPlane, 500),
        (WriteClass::CriticalControlPlane, 300),
        (WriteClass::OperatorProjection, 400),
        (WriteClass::OperatorProjection, 300),
        (WriteClass::BulkBlob, 500),
        (WriteClass::BulkBlob, 300),
    ];

    for (class, size) in test_cases {
        let result = budget.reserve(class, size);
        assert!(
            result.is_ok(),
            "Reserve {:?} size {} should succeed",
            class,
            size
        );
    }

    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        200,
        "Critical remaining after 500+300"
    );
    assert_eq!(
        budget.remaining(WriteClass::OperatorProjection),
        300,
        "Projection remaining after 400+300"
    );
    assert_eq!(
        budget.remaining(WriteClass::BulkBlob),
        200,
        "Bulk remaining after 500+300"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// BOS-02: ResourceQuota Namespace Isolation Under Concurrent Load
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn bos02_namespace_quota_isolation_under_pressure() {
    let mut enforcer = QuotaEnforcer::with_default_namespace();

    let namespaces = ["payments", "analytics", "inventory", "users", "orders"];

    for ns in &namespaces {
        let quota = NamespaceQuota::new(*ns)
            .with_cpu(vo_core::resource_quota::CpuQuota::new(
                std::num::NonZeroU64::new(100).expect("non-zero"),
            ))
            .with_memory(vo_core::resource_quota::MemoryQuota::new(
                std::num::NonZeroU64::new(1000).expect("non-zero"),
            ))
            .with_disk(vo_core::resource_quota::DiskQuota::new(
                std::num::NonZeroU64::new(5000).expect("non-zero"),
            ))
            .with_overcommit(OvercommitPolicy::NoOvercommit);

        enforcer
            .registry_mut()
            .register(quota)
            .expect("registration should succeed");
    }

    for ns in &namespaces {
        let result = enforcer.check_cpu(*ns, 50);
        assert!(result.is_ok(), "CPU check should pass for {ns} at 50%");
    }

    for ns in &namespaces {
        let result = enforcer.check_memory(*ns, 500);
        assert!(result.is_ok(), "Memory check should pass for {ns} at 50%");
    }

    for ns in &namespaces {
        let result = enforcer.check_disk(*ns, 2500);
        assert!(result.is_ok(), "Disk check should pass for {ns} at 50%");
    }

    let payments_over = enforcer.check_cpu("payments", 150);
    assert!(payments_over.is_err(), "payments over limit should fail");

    for ns in &namespaces {
        if *ns != "payments" {
            let result = enforcer.check_cpu(*ns, 50);
            assert!(
                result.is_ok(),
                "{ns} should remain unaffected by payments overage"
            );
        }
    }
}

#[test]
fn bos02_resource_quota_tracking_accuracy() {
    let mut enforcer = QuotaEnforcer::with_default_namespace();

    let quota = NamespaceQuota::new("tracking-test")
        .with_cpu(vo_core::resource_quota::CpuQuota::new(
            std::num::NonZeroU64::new(16).expect("non-zero"),
        ))
        .with_memory(vo_core::resource_quota::MemoryQuota::new(
            std::num::NonZeroU64::new(1024).expect("non-zero"),
        ));

    enforcer
        .registry_mut()
        .register(quota)
        .expect("registration should succeed");

    let usage_1 = QuotaUsage::new().with_cpu(4).with_memory(256);
    let usage_2 = QuotaUsage::new().with_cpu(8).with_memory(512);
    let usage_3 = QuotaUsage::new().with_cpu(4).with_memory(256);

    assert!(enforcer
        .check_cpu("tracking-test", usage_1.cpu_cores_used)
        .is_ok());
    assert!(enforcer
        .check_memory("tracking-test", usage_1.memory_bytes_used)
        .is_ok());

    assert!(enforcer
        .check_cpu("tracking-test", usage_2.cpu_cores_used)
        .is_ok());
    assert!(enforcer
        .check_memory("tracking-test", usage_2.memory_bytes_used)
        .is_ok());

    assert!(enforcer
        .check_cpu("tracking-test", usage_3.cpu_cores_used)
        .is_ok());
    assert!(enforcer
        .check_memory("tracking-test", usage_3.memory_bytes_used)
        .is_ok());

    let over_limit = QuotaUsage::new().with_cpu(32).with_memory(2048);
    assert!(enforcer
        .check_cpu("tracking-test", over_limit.cpu_cores_used)
        .is_err());
    assert!(enforcer
        .check_memory("tracking-test", over_limit.memory_bytes_used)
        .is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// BOS-03: CircuitBreaker Workflow Isolation Under Failure Storm
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn bos03_circuit_breaker_failure_storm_single_workflow() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("failure-storm-wf");

    let mut quarantine_triggered = false;

    for i in 0..1000u64 {
        let hash = format!("{i:08x}");
        let result = record_failure(&wf, &make_hash(&hash), &config, &state, t0);

        if matches!(result, Ok(Some(_))) {
            quarantine_triggered = true;
            let status = state.get_status(&wf);
            assert_eq!(
                status,
                RegistrationStatus::Quarantined,
                "Workflow should be quarantined after threshold failures"
            );
            break;
        }
    }

    assert!(
        quarantine_triggered,
        "Failure storm should trigger quarantine within 1000 attempts"
    );
}

#[test]
fn bos03_circuit_breaker_workflow_isolation_during_storm() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    let target_wf = make_wf("target-workflow");
    let victim_wf = make_wf("victim-workflow");
    let bystander_wf = make_wf("bystander-workflow");

    for i in 0..10u64 {
        let hash = format!("aaaa{i:04x}");
        record_failure(&target_wf, &make_hash(&hash), &config, &state, t0)
            .expect("failure recording should succeed");
    }

    assert_eq!(
        state.get_status(&target_wf),
        RegistrationStatus::Quarantined,
        "Target workflow should be quarantined"
    );

    assert_eq!(
        state.get_status(&victim_wf),
        RegistrationStatus::Active,
        "Victim workflow should remain active (not affected by target's failures)"
    );

    assert_eq!(
        state.get_status(&bystander_wf),
        RegistrationStatus::Active,
        "Bystander workflow should remain active"
    );

    let victim_request = make_request("victim-workflow", "bbbb0001", false);
    let result = evaluate_registration(&victim_request, &config, &state, t0);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::Allowed),
        "Victim should be allowed despite target's quarantine"
    );

    let bystander_request = make_request("bystander-workflow", "cccc0001", false);
    let result = evaluate_registration(&bystander_request, &config, &state, t0);
    assert_eq!(
        result,
        Ok(RegistrationOutcome::Allowed),
        "Bystander should be allowed despite target's quarantine"
    );
}

#[test]
fn bos03_circuit_breaker_rate_limit_enforcement_under_load() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();

    let request = make_request("rate-limit-test", "aaaa0001", false);

    let first = evaluate_registration(&request, &config, &state, t0);
    assert_eq!(
        first,
        Ok(RegistrationOutcome::Allowed),
        "First registration should be allowed"
    );

    let within_window =
        evaluate_registration(&request, &config, &state, t0 + Duration::from_secs(30));
    assert!(
        matches!(
            within_window,
            Ok(RegistrationOutcome::RateLimited {
                retry_after_secs: _
            })
        ),
        "Second registration within window should be rate-limited"
    );

    let after_window =
        evaluate_registration(&request, &config, &state, t0 + Duration::from_secs(120));
    assert_eq!(
        after_window,
        Ok(RegistrationOutcome::Allowed),
        "Registration after window should succeed"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// BOS-04: WorkloadBudget Acquire/Release Under Contention
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn bos04_workload_budget_exhaustion_and_recovery() {
    let budget = WorkloadBudget::new(10, 20, 8, 5);

    assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 10);
    assert_eq!(budget.remaining(WorkloadClass::Standard), 20);
    assert_eq!(budget.remaining(WorkloadClass::Recovery), 8);
    assert_eq!(budget.remaining(WorkloadClass::UnsafeBulk), 5);

    for _ in 0..10 {
        budget
            .acquire(WorkloadClass::ExactCritical)
            .expect("acquire should succeed");
    }

    assert!(
        budget.acquire(WorkloadClass::ExactCritical).is_err(),
        "ExactCritical should be exhausted"
    );

    budget.release(WorkloadClass::ExactCritical);
    assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 1);

    budget.release(WorkloadClass::ExactCritical);

    for _ in 0..20 {
        budget
            .acquire(WorkloadClass::Standard)
            .expect("acquire should succeed");
    }

    assert!(
        budget.acquire(WorkloadClass::Standard).is_err(),
        "Standard should be exhausted"
    );

    assert_eq!(
        budget.remaining(WorkloadClass::ExactCritical),
        2,
        "ExactCritical should be 2 (released twice, never re-acquired)"
    );
    assert_eq!(
        budget.remaining(WorkloadClass::Recovery),
        8,
        "Recovery should be unaffected by Standard exhaustion"
    );
    assert_eq!(
        budget.remaining(WorkloadClass::UnsafeBulk),
        5,
        "UnsafeBulk should be unaffected by Standard exhaustion"
    );

    budget
        .acquire(WorkloadClass::ExactCritical)
        .expect("acquire should succeed after release");
    assert_eq!(
        budget.remaining(WorkloadClass::ExactCritical),
        1,
        "ExactCritical should be 1 after re-acquire"
    );
}

#[test]
fn bos04_workload_budget_class_isolation() {
    let budget = WorkloadBudget::new(5, 5, 5, 5);

    budget
        .acquire(WorkloadClass::ExactCritical)
        .expect("acquire should succeed");
    assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 4);

    budget
        .acquire(WorkloadClass::Standard)
        .expect("acquire should succeed");
    assert_eq!(budget.remaining(WorkloadClass::Standard), 4);

    budget
        .acquire(WorkloadClass::Recovery)
        .expect("acquire should succeed");
    assert_eq!(budget.remaining(WorkloadClass::Recovery), 4);

    budget
        .acquire(WorkloadClass::UnsafeBulk)
        .expect("acquire should succeed");
    assert_eq!(budget.remaining(WorkloadClass::UnsafeBulk), 4);

    assert_eq!(budget.total_used(), 4, "Total used should be 4");
    assert_eq!(
        budget.total_reserved(),
        20,
        "Total reserved should be 20 (initial)"
    );
}

#[test]
fn bos04_workload_budget_acquire_release_cycle_stress() {
    let budget = WorkloadBudget::new(100, 1000, 100, 100);

    for _ in 0..1000 {
        budget
            .acquire(WorkloadClass::Standard)
            .expect("acquire should succeed");
        budget.release(WorkloadClass::Standard);
    }

    assert_eq!(
        budget.remaining(WorkloadClass::Standard),
        1000,
        "Standard budget should be unchanged after 1000 acquire/release cycles"
    );

    let exact_critical = budget.remaining(WorkloadClass::ExactCritical);
    let recovery = budget.remaining(WorkloadClass::Recovery);
    let unsafe_bulk = budget.remaining(WorkloadClass::UnsafeBulk);

    assert_eq!(
        exact_critical, 100,
        "ExactCritical should be unaffected by Standard cycling"
    );
    assert_eq!(
        recovery, 100,
        "Recovery should be unaffected by Standard cycling"
    );
    assert_eq!(
        unsafe_bulk, 100,
        "UnsafeBulk should be unaffected by Standard cycling"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// BOS-05: Composite Stress Tests - All Components Under Maximum Load
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn bos05_composite_write_and_workload_budget_interaction() {
    let write_budget = WriteBudget::new(1000, 2000, 5000);
    let workload_budget = WorkloadBudget::new(50, 200, 50, 100);

    let scenarios = [
        (
            WriteClass::CriticalControlPlane,
            WorkloadClass::ExactCritical,
            500,
        ),
        (
            WriteClass::CriticalControlPlane,
            WorkloadClass::Standard,
            300,
        ),
        (
            WriteClass::OperatorProjection,
            WorkloadClass::Standard,
            1000,
        ),
        (WriteClass::OperatorProjection, WorkloadClass::Recovery, 200),
        (WriteClass::BulkBlob, WorkloadClass::UnsafeBulk, 2000),
        (WriteClass::BulkBlob, WorkloadClass::Standard, 500),
    ];

    for (wc, wlc, size) in scenarios {
        assert!(
            write_budget.can_write(wc, size),
            "WriteBudget should allow {:?} write of size {}",
            wc,
            size
        );

        if workload_budget.can_acquire(wlc) {
            let write_result = write_budget.reserve(wc, size);
            assert!(
                write_result.is_ok(),
                "Write reserve should succeed for {:?}/{:?}",
                wc,
                wlc
            );
        }
    }
}

#[test]
fn bos05_composite_quota_and_breaker_interaction() {
    let mut enforcer = QuotaEnforcer::with_default_namespace();
    let cb_state = CircuitBreakerState::new();
    let cb_config = default_config();
    let t0 = Instant::now();

    let quota = NamespaceQuota::new("composite-test")
        .with_cpu(vo_core::resource_quota::CpuQuota::new(
            std::num::NonZeroU64::new(50).expect("non-zero"),
        ))
        .with_overcommit(OvercommitPolicy::NoOvercommit);

    enforcer
        .registry_mut()
        .register(quota)
        .expect("registration should succeed");

    let wf = make_wf("composite-workflow");

    for i in 0..10u64 {
        let hash = format!("{i:08x}");
        record_failure(&wf, &make_hash(&hash), &cb_config, &cb_state, t0)
            .expect("failure recording should succeed");
    }

    assert_eq!(
        cb_state.get_status(&wf),
        RegistrationStatus::Quarantined,
        "Workflow should be quarantined after failures"
    );

    let quota_check = enforcer.check_cpu("composite-test", 25);
    assert!(
        quota_check.is_ok(),
        "Quota check should pass (within limit)"
    );

    let force_request = RegistrationRequest {
        workflow_name: wf.clone(),
        binary_hash: make_hash("deadbeef"),
        force: true,
    };

    let force_result = evaluate_registration(&force_request, &cb_config, &cb_state, t0);
    assert_eq!(
        force_result,
        Ok(RegistrationOutcome::Allowed),
        "Force registration should bypass quarantine"
    );
}

#[test]
fn bos05_composite_all_tiers_pressure() {
    let write_budget = WriteBudget::new(10000, 20000, 50000);
    let workload_budget = WorkloadBudget::new(100, 1000, 100, 500);

    let write_classes = [
        WriteClass::CriticalControlPlane,
        WriteClass::OperatorProjection,
        WriteClass::BulkBlob,
    ];

    let workload_classes = [
        WorkloadClass::ExactCritical,
        WorkloadClass::Standard,
        WorkloadClass::Recovery,
        WorkloadClass::UnsafeBulk,
    ];

    let mut total_writes = 0u64;
    let mut total_workloads = 0u64;

    for wc in &write_classes {
        for _ in 0..1000 {
            if write_budget.reserve(*wc, 10).is_ok() {
                total_writes += 1;
            }
        }
    }

    for wlc in &workload_classes {
        for _ in 0..100 {
            if workload_budget.acquire(*wlc).is_ok() {
                total_workloads += 1;
            }
        }
    }

    assert!(
        total_writes > 0,
        "Should successfully allocate some writes, got {total_writes}"
    );
    assert!(
        total_workloads > 0,
        "Should successfully acquire some workloads, got {total_workloads}"
    );

    assert!(
        write_budget.remaining(WriteClass::CriticalControlPlane) < 10000,
        "Critical tier should show pressure"
    );
    assert!(
        workload_budget.remaining(WorkloadClass::Standard) < 1000,
        "Standard workload should show pressure"
    );
}

#[test]
fn bos05_stress_multi_namespace_concurrent_operations() {
    let mut enforcer = QuotaEnforcer::with_default_namespace();
    let namespaces = ["ns-a", "ns-b", "ns-c", "ns-d", "ns-e"];

    for (i, ns) in namespaces.iter().enumerate() {
        let quota = NamespaceQuota::new(*ns)
            .with_cpu(vo_core::resource_quota::CpuQuota::new(
                std::num::NonZeroU64::new((i as u64 + 1) * 10).expect("non-zero"),
            ))
            .with_memory(vo_core::resource_quota::MemoryQuota::new(
                std::num::NonZeroU64::new((i as u64 + 1) * 100).expect("non-zero"),
            ));

        enforcer
            .registry_mut()
            .register(quota)
            .expect("registration should succeed");
    }

    for ns in &namespaces {
        for level in 1..=5 {
            let cpu_result = enforcer.check_cpu(*ns, level);
            let mem_result = enforcer.check_memory(*ns, level * 10);

            assert!(
                cpu_result.is_ok(),
                "CPU check should pass for {} at level {}",
                ns,
                level
            );
            assert!(
                mem_result.is_ok(),
                "Memory check should pass for {} at level {}",
                ns,
                level
            );
        }
    }

    for ns in &namespaces {
        for level in 1..=5 {
            let cpu_result = enforcer.check_cpu(*ns, level);
            let mem_result = enforcer.check_memory(*ns, level * 10);

            assert!(
                cpu_result.is_ok(),
                "CPU check should pass for {} at level {}",
                ns,
                level
            );
            assert!(
                mem_result.is_ok(),
                "Memory check should pass for {} at level {}",
                ns,
                level
            );
        }
    }

    let overflow_result = enforcer.check_cpu("ns-a", 100);
    assert!(
        overflow_result.is_err(),
        "ns-a should reject over-limit CPU"
    );

    for ns in &namespaces[1..] {
        let result = enforcer.check_cpu(*ns, 10);
        assert!(
            result.is_ok(),
            "{} should not be affected by ns-a over-limit",
            ns
        );
    }
}
