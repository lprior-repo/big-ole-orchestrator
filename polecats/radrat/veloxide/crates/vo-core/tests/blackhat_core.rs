//! BLACK-HAT adversarial stress tests for vo-core.
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Barrier;
use vo_core::admission::{
    check_admission_with_thresholds, AdmissionThresholds, WritePressureState,
};
use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, CircuitBreakerConfig, CircuitBreakerState,
    RegistrationOutcome, RegistrationRequest,
};
use vo_core::replay::ReplayEngine;
use vo_types::events::EventEnvelope;

fn req(wf: &str, h: &str) -> RegistrationRequest {
    RegistrationRequest {
        workflow_name: vo_types::WorkflowName::parse(wf).unwrap(),
        binary_hash: vo_types::BinaryHash::parse(h).unwrap(),
        force: false,
    }
}
fn ev(id: &str, seq: u64) -> EventEnvelope {
    serde_json::from_value(serde_json::json!({
        "schema_version":1,"instance_id":id,"sequence":seq,"timestamp_ms":1000*seq,
        "payload":{"type":"WorkflowStarted","workflow_id":"wf","binary_hash":"h",
            "workflow_version_hash":"wv","dedupe_key_hash":null,"version":1},"metadata":{}}))
    .unwrap()
}
fn ok_thresh() -> AdmissionThresholds {
    AdmissionThresholds {
        writer_queue_depth_threshold: 1000,
        batch_commit_latency_ms_threshold: 5000,
        blob_queue_depth_threshold: 1000,
    }
}

#[test]
fn bh001_circuit_breaker_storm() {
    let s = Arc::new(CircuitBreakerState::new());
    let c = CircuitBreakerConfig::new(Duration::from_secs(1), Duration::from_secs(5), 3).unwrap();
    let n = Instant::now();
    for i in 0..50u32 {
        let wf = format!("x{i:04x}");
        for a in 0..5 {
            record_failure(
                &vo_types::WorkflowName::parse(&wf).unwrap(),
                &vo_types::BinaryHash::parse(&format!("{i:08x}{a:04x}")).unwrap(),
                &c,
                &s,
                n + Duration::from_millis(a),
            )
            .unwrap();
        }
        assert!(
            matches!(
                evaluate_registration(&req(&wf, &format!("{i:08x}0000")), &c, &s, n).unwrap(),
                RegistrationOutcome::WorkflowQuarantined { .. }
            ),
            "{wf} not quarantined"
        );
    }
}

#[test]
fn bh002_admission_rejects_extreme() {
    assert!(check_admission_with_thresholds(
        &WritePressureState {
            writer_queue_depth: u64::MAX,
            batch_commit_latency_ms: u64::MAX,
            blob_queue_depth: u64::MAX,
            compaction_stall_active: true,
            storage_stall_active: true
        },
        &ok_thresh()
    )
    .is_err());
}

#[test]
fn bh003_admission_allows_zero() {
    assert!(check_admission_with_thresholds(
        &WritePressureState {
            writer_queue_depth: 0,
            batch_commit_latency_ms: 0,
            blob_queue_depth: 0,
            compaction_stall_active: false,
            storage_stall_active: false
        },
        &ok_thresh()
    )
    .is_ok());
}

#[test]
fn bh004_replay_cross_instance() {
    let e: Vec<EventEnvelope> = (0..100)
        .map(|i| ev(if i < 50 { "a" } else { "b" }, (i + 1) as u64))
        .collect();
    assert!(ReplayEngine::new().replay(&e).is_err());
}

#[test]
fn bh005_replay_sequence_gap() {
    assert!(ReplayEngine::new()
        .replay(&[ev("x", 1), ev("x", 100)])
        .is_err());
}

#[test]
fn bh006_dedup_no_inflation() {
    let c = CircuitBreakerConfig::new(Duration::from_secs(5), Duration::from_secs(60), 3).unwrap();
    let s = CircuitBreakerState::new();
    let n = Instant::now();
    let w = vo_types::WorkflowName::parse("d").unwrap();
    let h = vo_types::BinaryHash::parse("deadbeef").unwrap();
    for _ in 0..100 {
        record_failure(&w, &h, &c, &s, n).ok();
    }
    assert!(!matches!(
        evaluate_registration(&req("d", "deadbeef"), &c, &s, n).unwrap(),
        RegistrationOutcome::WorkflowQuarantined { .. }
    ));
}

#[tokio::test]
async fn bh007_concurrent_admission() {
    let b = Arc::new(Barrier::new(20));
    let h: Vec<_> = (0..20)
        .map(|_| {
            let b = b.clone();
            tokio::spawn(async move {
                b.wait().await;
                let _ = check_admission_with_thresholds(
                    &WritePressureState {
                        writer_queue_depth: 500,
                        batch_commit_latency_ms: 500,
                        blob_queue_depth: 500,
                        compaction_stall_active: false,
                        storage_stall_active: false,
                    },
                    &ok_thresh(),
                );
            })
        })
        .collect();
    for x in h {
        x.await.unwrap();
    }
}

#[test]
fn bh008_min_window_quarantine() {
    let c =
        CircuitBreakerConfig::new(Duration::from_millis(1), Duration::from_secs(60), 1).unwrap();
    let s = CircuitBreakerState::new();
    let n = Instant::now();
    record_failure(
        &vo_types::WorkflowName::parse("q").unwrap(),
        &vo_types::BinaryHash::parse("00000001").unwrap(),
        &c,
        &s,
        n,
    )
    .unwrap();
    assert!(matches!(
        evaluate_registration(&req("q", "00000001"), &c, &s, n).unwrap(),
        RegistrationOutcome::WorkflowQuarantined { .. }
    ));
}

#[test]
fn bh009_replay_empty_idempotent() {
    let e = ReplayEngine::new();
    for _ in 0..100 {
        assert_eq!(e.replay(&[]).unwrap().events_applied, 0);
    }
}

#[test]
fn bh010_replay_single_consistent() {
    let e = ReplayEngine::new();
    let v = ev("t", 1);
    for _ in 0..50 {
        assert_eq!(e.replay(&[v.clone()]).unwrap().events_applied, 1);
    }
}

#[test]
fn bh011_admission_boundary() {
    assert!(check_admission_with_thresholds(
        &WritePressureState {
            writer_queue_depth: 999,
            batch_commit_latency_ms: 4999,
            blob_queue_depth: 999,
            compaction_stall_active: false,
            storage_stall_active: false
        },
        &ok_thresh()
    )
    .is_ok());
}
