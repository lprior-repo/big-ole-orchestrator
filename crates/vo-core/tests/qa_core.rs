//! QA tests for replay engine, admission control, and circuit breaker.
//!
//! Exercises composed behavior across the three core subsystems:
//! - Replay: deterministic event reconstruction, sequence validation
//! - Admission: write-pressure coupling, degraded mode
//! - Circuit breaker: quarantine lifecycle, failure window, rate limiting

use std::time::{Duration, Instant};

use vo_core::admission::{
    check_admission, check_admission_with_thresholds, AdmissionError, AdmissionThresholds,
    WritePressureState,
};
use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, unquarantine, CircuitBreakerConfig, CircuitBreakerState,
    RegistrationOutcome, RegistrationRequest, RegistrationStatus,
};
use vo_core::replay::{ReplayEngine, ReplayError};
use vo_types::events::EventEnvelope;

#[test]
fn replay_rejects_empty_events_gracefully() {
    let engine = ReplayEngine::new();
    let result = engine.replay(&[]);
    assert!(result.is_ok());
    let r = result.unwrap();
    assert_eq!(r.events_applied, 0);
    assert!(r.final_state.is_none());
}

#[test]
fn replay_detects_instance_id_mismatch() {
    let engine = ReplayEngine::new();
    let events = vec![
        serde_json::from_value::<EventEnvelope>(serde_json::json!({
            "schema_version": 1, "instance_id": "inst-1", "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf",
                "binary_hash": "h", "workflow_version_hash": "wv",
                "dedupe_key_hash": null, "version": 1},
            "metadata": {}
        }))
        .unwrap(),
        serde_json::from_value::<EventEnvelope>(serde_json::json!({
            "schema_version": 1, "instance_id": "inst-2", "sequence": 2,
            "timestamp_ms": 2000,
            "payload": {"type": "StepScheduled", "workflow_id": "wf",
                "step_id": "s1", "attempt": 1, "fence": 1,
                "execution_id": "e1", "version": 1},
            "metadata": {}
        }))
        .unwrap(),
    ];
    let err = engine.replay(&events).unwrap_err();
    assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
}

#[test]
fn replay_detects_sequence_gap() {
    let engine = ReplayEngine::new();
    let id = "inst-gap";
    let mk = |seq| {
        serde_json::from_value::<EventEnvelope>(serde_json::json!({
            "schema_version": 1, "instance_id": id, "sequence": seq,
            "timestamp_ms": 1000 * seq,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf",
                "binary_hash": "h", "workflow_version_hash": "wv",
                "dedupe_key_hash": null, "version": 1},
            "metadata": {}
        }))
        .unwrap()
    };
    let events = vec![mk(1), mk(3)]; // gap at sequence 2
    let err = engine.replay(&events).unwrap_err();
    assert!(matches!(err, ReplayError::SequenceGap { .. }));
}

// ── Admission Control ──────────────────────────────────────────────────────────

#[test]
fn admission_allows_healthy_pressure() {
    let state = WritePressureState::default();
    assert!(check_admission(&state).is_ok());
}

#[test]
fn admission_rejects_writer_queue_overflow() {
    let state = WritePressureState {
        writer_queue_depth: 200,
        ..Default::default()
    };
    let err = check_admission(&state).unwrap_err();
    assert!(matches!(
        err,
        AdmissionError::WriterQueueDepthExceeded { .. }
    ));
}

#[test]
fn admission_rejects_multiple_indicators() {
    let state = WritePressureState {
        writer_queue_depth: 200,
        batch_commit_latency_ms: 2000,
        blob_queue_depth: 100,
        compaction_stall_active: true,
        storage_stall_active: true,
    };
    let err = check_admission(&state).unwrap_err();
    assert!(matches!(
        err,
        AdmissionError::MultiplePressureIndicators { .. }
    ));
    if let AdmissionError::MultiplePressureIndicators { indicators } = err {
        assert_eq!(indicators.len(), 5);
    }
}

#[test]
fn admission_custom_thresholds_boundary() {
    let state = WritePressureState {
        writer_queue_depth: 50,
        batch_commit_latency_ms: 100,
        blob_queue_depth: 10,
        compaction_stall_active: false,
        storage_stall_active: false,
    };
    let tight = AdmissionThresholds {
        writer_queue_depth_threshold: 50,
        batch_commit_latency_ms_threshold: 100,
        blob_queue_depth_threshold: 10,
    };
    assert!(check_admission_with_thresholds(&state, &tight).is_ok());
    let tighter = AdmissionThresholds {
        writer_queue_depth_threshold: 49,
        ..tight.clone()
    };
    assert!(matches!(
        check_admission_with_thresholds(&state, &tighter),
        Err(AdmissionError::WriterQueueDepthExceeded { .. })
    ));
}

// ── Circuit Breaker ────────────────────────────────────────────────────────────

#[test]
fn circuit_breaker_allows_fresh_registration() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(1), Duration::from_secs(60), 3).unwrap();
    let req = RegistrationRequest {
        workflow_name: vo_types::WorkflowName::parse("wf-test").unwrap(),
        binary_hash: vo_types::BinaryHash::parse("aabbccdd").unwrap(),
        force: None,
    };
    let outcome = evaluate_registration(&req, &config, &state, Instant::now()).unwrap();
    assert_eq!(outcome, RegistrationOutcome::Allowed);
}

#[test]
fn circuit_breaker_force_bypasses_quarantine() {
    let state = CircuitBreakerState::new();
    state.statuses.insert(
        vo_types::WorkflowName::parse("wf-q").unwrap(),
        RegistrationStatus::Quarantined,
    );
    state.register_operator_token("test-operator-token".into());
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(1), Duration::from_secs(60), 3).unwrap();
    let req = RegistrationRequest {
        workflow_name: vo_types::WorkflowName::parse("wf-q").unwrap(),
        binary_hash: vo_types::BinaryHash::parse("aabbccdd").unwrap(),
        force: Some("test-operator-token".into()),
    };
    let outcome = evaluate_registration(&req, &config, &state, Instant::now()).unwrap();
    assert_eq!(outcome, RegistrationOutcome::Allowed);
}

#[test]
fn circuit_breaker_quarantine_after_threshold() {
    let state = CircuitBreakerState::new();
    let config =
        CircuitBreakerConfig::new(Duration::from_secs(1), Duration::from_secs(60), 4).unwrap();
    let wf = vo_types::WorkflowName::parse("wf-fail").unwrap();
    let now = Instant::now();
    for i in 0..3u8 {
        let hash = vo_types::BinaryHash::parse(&format!("aabbccd{i}")).unwrap();
        let event = record_failure(&wf, &hash, &config, &state, now).unwrap();
        assert!(event.is_none(), "no quarantine before threshold at i={i}");
    }
    let hash3 = vo_types::BinaryHash::parse("aabbccd3").unwrap();
    let event = record_failure(&wf, &hash3, &config, &state, now).unwrap();
    assert!(
        event.is_some(),
        "quarantine after threshold (4th unique hash)"
    );
    assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
}
