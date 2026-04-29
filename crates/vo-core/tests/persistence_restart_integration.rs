//! Cross-crate integration test: quarantine survives Fjall restart.
//!
//! This test proves quarantine persisted via vo-storage survives a simulated
//! engine restart AND blocks registration via vo-core's evaluate_registration.
//!
//! Originally in vo-storage's persistence_integration.rs, moved here because
//! vo-storage's structural test disallows vo-core as a dependency.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    evaluate_registration, CircuitBreakerConfig, CircuitBreakerState, RegistrationOutcome,
    RegistrationRequest,
};
use vo_storage::status_store::{load_all_statuses, write_registration_status, WORKFLOWS_PARTITION};
use vo_types::{BinaryHash, RegistrationStatus, WorkflowName};

fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

fn setup_partition() -> (tempfile::TempDir, fjall::Database, fjall::Keyspace) {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    let db = fjall::Database::builder(dir.path())
        .open()
        .expect("keyspace should open");
    let partition = db
        .keyspace(WORKFLOWS_PARTITION, fjall::KeyspaceCreateOptions::default)
        .expect("partition should open");
    (dir, db, partition)
}

#[test]
fn quarantine_blocks_registration_after_restart_hydration() {
    let (_dir, _ks, partition) = setup_partition();
    let wf = make_wf("deploy-prod");

    // Phase 1: Write quarantined status
    write_registration_status(&partition, &wf, RegistrationStatus::Quarantined)
        .expect("write should succeed");

    // Phase 2: Simulate restart — load from Fjall, hydrate DashMap state
    let loaded = load_all_statuses(&partition).expect("load should succeed");

    let state = CircuitBreakerState::new();
    loaded.iter().for_each(|(workflow_name, status)| {
        state.statuses.insert(workflow_name.clone(), *status);
    });

    // Phase 3: Try to register — should be blocked by quarantine
    let config = CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5)
        .expect("config should be valid");

    let request = RegistrationRequest {
        workflow_name: wf.clone(),
        binary_hash: BinaryHash::parse("abcdef01").expect("valid hash"),
        force: None,
    };

    let result = evaluate_registration(&request, &config, &state, Instant::now());
    assert_eq!(
        result,
        Ok(RegistrationOutcome::WorkflowQuarantined { workflow_name: wf })
    );
}
