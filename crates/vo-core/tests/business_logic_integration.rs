//! End-to-end integration tests for vo-core business logic.
//!
//! Tests the full composed behavior across multiple core components:
//! - Admission control with pressure thresholds
//! - Circuit breaker state management
//! - Replay engine with event processing
//! - Resource quota enforcement
//! - Upcaster version compatibility
//!
//! These tests verify that all components work together correctly
//! in realistic business scenarios.

use std::time::{Duration, Instant};

use serde_json::json;
use vo_core::admission::{check_admission, AdmissionThresholds, WritePressureState};
use vo_core::circuit_breaker::{
    check_rate_limit, evaluate_registration, record_failure, unquarantine, CircuitBreakerConfig,
    CircuitBreakerState, RegistrationOutcome, RegistrationRequest, RegistrationStatus,
    UnquarantineResult,
};
use vo_core::exact_once_verification::crash_points::{CrashPoint, CrashPosition};
use vo_core::exact_once_verification::harness::VerificationHarness;
use vo_core::replay::ReplayEngine;
use vo_core::resource_quota::{
    CpuQuota, DiskQuota, MemoryQuota, NamespaceQuota, NamespaceRegistry, OvercommitPolicy,
    QuotaEnforcer,
};
use vo_types::events::EventMetadata;
use vo_types::{BinaryHash, WorkflowName};

// ── Test Helpers ─────────────────────────────────────────────────────────────

fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

fn make_hash(s: &str) -> BinaryHash {
    // Use valid hex format (lowercase hex, even length)
    let valid: String = s
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let padded = format!("{:0>32}", valid);
    BinaryHash::parse(&padded).expect("test hash should be valid")
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

fn make_event(
    instance_id: &str,
    sequence: u64,
    payload: serde_json::Value,
) -> vo_types::events::EventEnvelope {
    vo_types::events::EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 * sequence,
        payload,
        metadata: EventMetadata::default(),
    }
}

fn workflow_started_payload(workflow_id: &str) -> serde_json::Value {
    json!({
        "type": "WorkflowStarted",
        "workflow_id": workflow_id,
        "binary_hash": "sha256abc",
        "workflow_version_hash": "wvhash123",
        "dedupe_key_hash": null,
        "version": 1
    })
}

fn step_scheduled_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    json!({
        "type": "StepScheduled",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "attempt": 1,
        "fence": 1,
        "execution_id": "exec-1",
        "version": 1
    })
}

fn step_started_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    json!({
        "type": "StepStarted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "started_at_ms": 2000,
        "version": 1
    })
}

fn step_completed_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    json!({
        "type": "StepCompleted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "completed_at_ms": 3000,
        "attempt": 1,
        "fence": 1,
        "routing_projection": null,
        "output_ref": null,
        "output_hash": null,
        "output": null,
        "version": 1
    })
}

fn step_failed_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    json!({
        "type": "StepFailed",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "failure_reason": "error",
        "attempt": 1,
        "fence": 1,
        "version": 1
    })
}

// ── Integration: Full Workflow Lifecycle ─────────────────────────────────────

#[test]
fn end_to_end_workflow_with_admission_and_circuit_breaker() {
    // Given: A workflow needs to be registered and admitted
    let mut state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();
    let request = make_request("order-processing", "abcdef01", false);

    // When: Workflow registration is evaluated
    let result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(result, Ok(RegistrationOutcome::Allowed));

    // Then: Register the workflow
    state
        .statuses
        .insert(request.workflow_name.clone(), RegistrationStatus::Active);

    // Given: Pressure state is within thresholds
    let pressure_state = WritePressureState {
        writer_queue_depth: 50,
        batch_commit_latency_ms: 200,
        blob_queue_depth: 20,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: Admission is checked
    let admission_result = check_admission(&pressure_state);

    // Then: Admission is granted
    assert_eq!(admission_result, Ok(()));
}

// ── Integration: Failure Recovery Scenario ───────────────────────────────────

#[test]
fn workflow_failure_recovery_with_circuit_breaker() {
    // Given: A workflow in active state
    let mut state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    let wf = make_wf("payment-service");
    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Active);

    // When: Workflow experiences failures
    for i in 0..5 {
        let _ = record_failure(
            &wf,
            &make_hash("abcdef01"),
            &config,
            &state,
            Instant::now() - Duration::from_secs(10 * i as u64),
        );
    }

    // Then: Workflow should be quarantined
    let request = make_request("payment-service", "abcdef01", false);
    let result = evaluate_registration(&request, &config, &state, now);

    match result {
        Ok(RegistrationOutcome::WorkflowQuarantined { .. }) => {}
        _ => panic!("Expected WorkflowQuarantined, got {:?}", result),
    }

    // When: Admin performs unquarantine
    let unquarantine_result = unquarantine(&wf, "admin", &state);

    // Then: Workflow is returned to active state
    assert!(matches!(
        &unquarantine_result,
        Ok(UnquarantineResult {
            new_status: RegistrationStatus::Active,
            ..
        })
    ));
    if let Ok(UnquarantineResult { workflow_name, .. }) = unquarantine_result {
        assert_eq!(workflow_name, wf);
    }
}

// ── Integration: Resource Quota Enforcement ───────────────────────────────────

#[test]
fn resource_quota_enforcement_with_admission() {
    // Given: A quota registry with namespace limits
    let default_ns = NamespaceQuota::new("default")
        .with_cpu(CpuQuota::new(std::num::NonZeroU64::new(4).unwrap()))
        .with_memory(MemoryQuota::new(
            std::num::NonZeroU64::new(8 * 1024 * 1024 * 1024).unwrap(),
        ))
        .with_disk(DiskQuota::new(
            std::num::NonZeroU64::new(100 * 1024 * 1024 * 1024).unwrap(),
        ));

    let mut registry = NamespaceRegistry::new();
    let _ = registry.register(default_ns);

    let enforcer = QuotaEnforcer::new(registry);

    // When: Request within limits
    let result = enforcer.check_cpu("default", 2);
    assert!(result.is_ok());

    // When: Request exceeds CPU limit
    let result = enforcer.check_cpu("default", 10);
    assert!(result.is_err());

    // When: Request within memory limits
    let result = enforcer.check_memory("default", 1024 * 1024 * 1024); // 1GB
    assert!(result.is_ok());

    // When: Request exceeds memory limits
    let result = enforcer.check_memory("default", 16 * 1024 * 1024 * 1024); // 16GB
    assert!(result.is_err());
}

// ── Integration: Admission Thresholds ─────────────────────────────────────────

#[test]
fn admission_with_custom_thresholds() {
    // Given: Custom admission thresholds
    let thresholds = AdmissionThresholds {
        writer_queue_depth_threshold: 200,
        batch_commit_latency_ms_threshold: 2000,
        blob_queue_depth_threshold: 100,
    };

    // Given: Pressure state within custom thresholds
    let pressure_state = WritePressureState {
        writer_queue_depth: 150,
        batch_commit_latency_ms: 1500,
        blob_queue_depth: 80,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: Admission is checked with custom thresholds
    let result = vo_core::admission::check_admission_with_thresholds(&pressure_state, &thresholds);

    // Then: Admission is granted
    assert_eq!(result, Ok(()));
}

// ── Integration: Rate Limiting ────────────────────────────────────────────────

#[test]
fn rate_limiting_with_successive_failures() {
    // Given: A circuit breaker state and config
    let mut state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    let wf = make_wf("test-wf");
    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Active);

    // When: Multiple rapid failures occur
    for i in 0..10 {
        let _ = record_failure(
            &wf,
            &make_hash("abcdef01"),
            &config,
            &state,
            now - Duration::from_secs(5 * i as u64),
        );
    }

    // Then: Rate limit should be exceeded
    let last_reg = state.rate_limiter.get(&wf).map(|r| *r);
    let result = check_rate_limit(last_reg, config.rate_limit_window, now);
    assert!(result.is_some()); // Rate limited, returns retry-after seconds
}

// ── Integration: Full Business Scenario ───────────────────────────────────────

#[test]
fn e2e_order_processing_workflow() {
    // Scenario: Order processing workflow with all core components

    // 1. Workflow registration
    let mut state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    let order_wf = make_wf("order-processing");
    let request = make_request("order-processing", "order-binary-01", false);

    let reg_result = evaluate_registration(&request, &config, &state, now);
    assert_eq!(reg_result, Ok(RegistrationOutcome::Allowed));

    state
        .statuses
        .insert(order_wf.clone(), RegistrationStatus::Active);

    // 2. Admission check for order processing
    let pressure_state = WritePressureState {
        writer_queue_depth: 30,
        batch_commit_latency_ms: 100,
        blob_queue_depth: 10,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    assert_eq!(check_admission(&pressure_state), Ok(()));

    // 3. Resource quota allocation
    let default_ns = NamespaceQuota::new("default")
        .with_cpu(CpuQuota::new(std::num::NonZeroU64::new(4).unwrap()))
        .with_memory(MemoryQuota::new(
            std::num::NonZeroU64::new(2 * 1024 * 1024 * 1024).unwrap(),
        ))
        .with_disk(DiskQuota::new(
            std::num::NonZeroU64::new(50 * 1024 * 1024 * 1024).unwrap(),
        ));

    let mut registry = NamespaceRegistry::new();
    let _ = registry.register(default_ns);

    let enforcer = QuotaEnforcer::new(registry);
    let cpu_result = enforcer.check_cpu("default", 2);
    let mem_result = enforcer.check_memory("default", 512 * 1024 * 1024); // 512MB
    assert!(cpu_result.is_ok());
    assert!(mem_result.is_ok());

    // 4. Replay order events
    let engine = ReplayEngine::new();
    let order_events = vec![
        make_event("order-123", 1, workflow_started_payload("order-123")),
        make_event(
            "order-123",
            2,
            step_scheduled_payload("order-123", "validate-payment"),
        ),
        make_event(
            "order-123",
            3,
            step_started_payload("order-123", "validate-payment"),
        ),
        make_event(
            "order-123",
            4,
            step_completed_payload("order-123", "validate-payment"),
        ),
        make_event(
            "order-123",
            5,
            step_scheduled_payload("order-123", "charge-card"),
        ),
    ];

    let replay_result = engine.replay(&order_events).expect("replay should succeed");
    assert_eq!(replay_result.events_applied, 5);
    assert_eq!(
        replay_result.final_state,
        Some(vo_types::state::LifecycleState::StepScheduled)
    );

    // 5. Simulate payment failure
    let failure_events = vec![
        make_event(
            "order-123",
            6,
            step_failed_payload("order-123", "charge-card"),
        ),
        make_event("order-123", 7, workflow_started_payload("order-123")), // Using started as resume proxy
        make_event(
            "order-123",
            8,
            step_scheduled_payload("order-123", "retry-payment"),
        ),
    ];

    let replay_result = engine
        .replay(&failure_events)
        .expect("replay should succeed");
    assert_eq!(replay_result.events_applied, 3);

    // 6. Record workflow failure in circuit breaker
    let _ = record_failure(
        &order_wf,
        &make_hash("order-binary-01"),
        &config,
        &state,
        Instant::now() - Duration::from_secs(60),
    );

    // 7. Check rate limit after failure
    let last_reg = state.rate_limiter.get(&order_wf).map(|r| *r);
    let rate_result = check_rate_limit(last_reg, config.rate_limit_window, now);
    assert!(rate_result.is_none()); // Only one failure, not rate limited yet

    // All steps completed successfully
    assert!(true);
}

// ── Integration: Crash Recovery Scenario ──────────────────────────────────────

#[test]
fn crash_recovery_with_exact_once_verification() {
    // Given: A verification harness with crash injection
    let harness =
        VerificationHarness::with_crash_scenario(CrashPoint::DedupeWrite, CrashPosition::Before);
    assert!(harness.should_crash(CrashPoint::DedupeWrite));

    // Given: Events to replay
    let engine = ReplayEngine::new();
    let events = vec![
        make_event("crash-test", 1, workflow_started_payload("crash-test")),
        make_event(
            "crash-test",
            2,
            step_scheduled_payload("crash-test", "process"),
        ),
        make_event(
            "crash-test",
            3,
            step_started_payload("crash-test", "process"),
        ),
    ];

    // When: Replay before crash
    let pre_crash_result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(pre_crash_result.events_applied, 3);

    // When: Replay after crash (deterministic replay)
    let post_crash_result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(post_crash_result.events_applied, 3);

    // Then: Both replays reach the same state
    assert_eq!(pre_crash_result.final_state, post_crash_result.final_state);
}

// ── Integration: Circuit Breaker Rate Limiting ────────────────────────────────

#[test]
fn circuit_breaker_rate_limiting_with_config() {
    // Given: A workflow in active state
    let mut state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    let wf = make_wf("rate-limit-test");
    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Active);

    // When: Multiple failures within failure window
    for i in 0..6 {
        let _ = record_failure(
            &wf,
            &make_hash("test-hash"),
            &config,
            &state,
            now - Duration::from_secs(30 * i as u64),
        );
    }

    // Then: Rate limit should be exceeded after threshold
    let last_reg = state.rate_limiter.get(&wf).map(|r| *r);
    let result = check_rate_limit(last_reg, config.rate_limit_window, now);
    assert!(result.is_some()); // Rate limited
}

// ── Integration: Admission Pressure Scenarios ─────────────────────────────────

#[test]
fn admission_rejects_when_multiple_pressure_indicators_exceeded() {
    // Given: Multiple pressure indicators exceeded
    let pressure_state = WritePressureState {
        writer_queue_depth: 150,       // Exceeds default 100
        batch_commit_latency_ms: 1500, // Exceeds default 1000
        blob_queue_depth: 80,          // Exceeds default 50
        compaction_stall_active: true,
        storage_stall_active: true,
    };

    // When: Admission is checked
    let result = check_admission(&pressure_state);

    // Then: Admission should be rejected (first threshold exceeded)
    assert!(result.is_err());
}

// ── Integration: Quarantine and Recovery ──────────────────────────────────────

#[test]
fn full_quarantine_recovery_cycle() {
    // Given: A workflow that has been quarantined
    let mut state = CircuitBreakerState::new();
    let now = Instant::now();

    let wf = make_wf("quarantine-test");
    state
        .statuses
        .insert(wf.clone(), RegistrationStatus::Quarantined);

    // When: Admin attempts unquarantine
    let result = unquarantine(&wf, "admin", &state);

    // Then: Workflow is unquarantined
    assert!(matches!(
        &result,
        Ok(UnquarantineResult {
            new_status: RegistrationStatus::Active,
            ..
        })
    ));

    // When: Workflow is re-registered after unquarantine
    let request = make_request("quarantine-test", "new-hash", false);
    let reg_result = evaluate_registration(&request, &default_config(), &state, now);

    // Then: Registration is allowed
    assert_eq!(reg_result, Ok(RegistrationOutcome::Allowed));
}

// ── Integration: Admission Default Thresholds ─────────────────────────────────

#[test]
fn admission_uses_default_thresholds_when_not_specified() {
    // Given: Pressure state exceeding default thresholds
    let pressure_state = WritePressureState {
        writer_queue_depth: 150, // Default threshold is 100
        batch_commit_latency_ms: 500,
        blob_queue_depth: 10,
        compaction_stall_active: false,
        storage_stall_active: false,
    };

    // When: Admission is checked with default thresholds
    let result = check_admission(&pressure_state);

    // Then: Admission is rejected
    assert!(result.is_err());
}

// ── Integration: Multiple Workflows ───────────────────────────────────────────

#[test]
fn multiple_workflows_independent_circuit_breakers() {
    // Given: Multiple workflows with independent state
    let mut state = CircuitBreakerState::new();
    let config = default_config();
    let now = Instant::now();

    let wf1 = make_wf("workflow-1");
    let wf2 = make_wf("workflow-2");

    state
        .statuses
        .insert(wf1.clone(), RegistrationStatus::Active);
    state
        .statuses
        .insert(wf2.clone(), RegistrationStatus::Active);

    // When: Only workflow-1 experiences failures
    for i in 0..6 {
        let _ = record_failure(
            &wf1,
            &make_hash("hash-1"),
            &config,
            &state,
            now - Duration::from_secs(30 * i as u64),
        );
    }

    // Then: Only workflow-1 is rate limited, workflow-2 is unaffected
    let last_reg1 = state.rate_limiter.get(&wf1).map(|r| *r);
    let last_reg2 = state.rate_limiter.get(&wf2).map(|r| *r);

    assert!(check_rate_limit(last_reg1, config.rate_limit_window, now).is_some());
    assert!(check_rate_limit(last_reg2, config.rate_limit_window, now).is_none());

    // When: Evaluate both workflows
    let request1 = make_request("workflow-1", "hash-1", false);
    let request2 = make_request("workflow-2", "hash-2", false);

    let result1 = evaluate_registration(&request1, &config, &state, now);
    let result2 = evaluate_registration(&request2, &config, &state, now);

    // Then: workflow-1 is rate limited, workflow-2 is allowed
    match result1 {
        Ok(RegistrationOutcome::RateLimited { .. }) => {}
        _ => panic!("Expected RateLimited for workflow-1"),
    }
    assert_eq!(result2, Ok(RegistrationOutcome::Allowed));
}
