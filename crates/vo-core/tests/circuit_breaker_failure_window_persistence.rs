//! BDD test for ADR-026: Persist AI deployment failure window across restart.
//!
//! Given workflow W records failed versions inside failure window
//! When runtime restarts
//! Then failure history is recovered and continues counting toward quarantine

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    record_failure, CircuitBreakerConfig, CircuitBreakerState, FailureWindow, RegistrationStatus,
};
use vo_storage::failure_window_store::{
    load_all_failure_windows, persist_failure_window, FailureRecordView, FAILURE_WINDOWS_PARTITION,
};
use vo_types::{BinaryHash, WorkflowName};

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

fn setup_fjall() -> (tempfile::TempDir, fjall::Database, fjall::Keyspace) {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = fjall::Database::builder(dir.path())
        .open()
        .expect("fjall open");
    let partition = db
        .keyspace(
            FAILURE_WINDOWS_PARTITION,
            fjall::KeyspaceCreateOptions::default,
        )
        .expect("partition create");
    (dir, db, partition)
}

#[test]
fn given_deployment_failures_when_runtime_restarts_then_failure_window_is_persisted() {
    let config = default_config();
    let wf = make_wf("ai-deploy-prod");
    let (_dir, _db, partition) = setup_fjall();

    // ── GIVEN: workflow W records 3 failed versions inside failure window ──
    let state_v1 = CircuitBreakerState::new();
    let now = Instant::now();
    let base_time = now - Duration::from_secs(30);

    for i in 0..3u8 {
        let hash = make_hash(&format!("{:08x}", i));
        let failed_at = base_time + Duration::from_secs(u64::from(i) * 10);
        let result = record_failure(&wf, &hash, &config, &state_v1, failed_at);
        assert!(
            result.is_ok(),
            "recording failure {i} should succeed: {:?}",
            result
        );
    }

    // Verify we have 3 failures tracked
    assert_eq!(state_v1.get_failure_count(&wf), 3);
    assert_eq!(
        state_v1.get_status(&wf),
        RegistrationStatus::Active,
        "should not be quarantined yet (only 3 failures, threshold is 5)"
    );

    // ── Persist failure window to Fjall (simulating graceful shutdown) ──
    let window_guard = state_v1.failure_tracker.get(&wf).expect("window exists");
    let records = window_guard.value().records();
    let persist_view: Vec<FailureRecordView> = records
        .iter()
        .map(|r| FailureRecordView {
            hash: r.hash.clone(),
            age: now.duration_since(r.failed_at),
        })
        .collect();
    drop(window_guard);

    persist_failure_window(&partition, &wf, &persist_view).expect("persist should succeed");

    // Also persist the quarantine status if not Active
    if state_v1.get_status(&wf) != RegistrationStatus::Active {
        vo_storage::status_store::write_registration_status(
            &partition,
            &wf,
            state_v1.get_status(&wf),
        )
        .expect("status persist should succeed");
    }

    // ── WHEN: runtime restarts (state is dropped and recreated) ──
    drop(state_v1);

    let state_v2 = CircuitBreakerState::new();
    let restart_now = Instant::now();

    // Load failure windows from Fjall into fresh state
    let all_windows =
        load_all_failure_windows(&partition, restart_now).expect("load should succeed");

    for (wf_name, records) in all_windows {
        let mut window = FailureWindow::new();
        for rec in &records {
            let failed_at = restart_now - rec.age;
            vo_core::circuit_breaker::failure_window::record_failure_in_window(
                &mut window,
                rec.hash.clone(),
                failed_at,
                config.failure_window,
            );
        }
        state_v2.failure_tracker.insert(wf_name, window);
    }

    // ── THEN: failure history is recovered and continues counting toward quarantine ──

    // Verify the 3 persisted failures were recovered
    assert_eq!(
        state_v2.get_failure_count(&wf),
        3,
        "failure window should have 3 records after restart"
    );
    assert_eq!(
        state_v2.get_status(&wf),
        RegistrationStatus::Active,
        "should still be active after restart"
    );

    // Record a 4th failure — should NOT quarantine (threshold is 5)
    let hash4 = make_hash("000000aa");
    let result4 = record_failure(&wf, &hash4, &config, &state_v2, restart_now);
    assert_eq!(
        result4,
        Ok(None),
        "4th failure should not trigger quarantine"
    );
    assert_eq!(state_v2.get_failure_count(&wf), 4);

    // Record the 5th failure — SHOULD quarantine
    let hash5 = make_hash("000000bb");
    let result5 = record_failure(&wf, &hash5, &config, &state_v2, restart_now);
    assert!(
        result5.is_ok() && result5.as_ref().unwrap().is_some(),
        "5th failure should trigger quarantine"
    );
    assert_eq!(
        state_v2.get_status(&wf),
        RegistrationStatus::Quarantined,
        "workflow should be quarantined after 5 failures"
    );
}
