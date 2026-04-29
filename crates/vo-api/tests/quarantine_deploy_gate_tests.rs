//! BDD tests for quarantine gating on automated deployment (ADR-026).
//!
//! Bead: tw-4y6h.17.9
//!
//! BDD scenario:
//!   Given workflow W is Quarantined
//!   When automated deployment request arrives
//!   Then request is rejected and no active version changes

use std::sync::Arc;
use std::time::Instant;
use vo_core::circuit_breaker::{
    evaluate_registration, CircuitBreakerConfig, CircuitBreakerState, RegistrationOutcome,
    RegistrationRequest, RegistrationStatus,
};
use vo_types::{BinaryHash, WorkflowName};

fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).unwrap()
}

fn make_name(s: &str) -> WorkflowName {
    WorkflowName::parse(s).unwrap()
}

// ─── BDD: Quarantine gates automated deployment ─────────────────────────────

#[test]
fn given_quarantined_workflow_when_automated_deploy_arrives_then_deploy_is_rejected() {
    // Given: workflow W is Quarantined
    let config = CircuitBreakerConfig::default_config().unwrap();
    let state = Arc::new(CircuitBreakerState::new());
    let wf = make_name("ai-deploy-loop");
    state.set_status(wf.clone(), RegistrationStatus::Quarantined);

    // Capture the active version hash before the deployment attempt
    let deploy_hash = make_hash("cccc3333dddd4444");

    // When: automated deployment request arrives (non-force, new binary hash)
    let request = RegistrationRequest {
        workflow_name: wf.clone(),
        binary_hash: deploy_hash.clone(),
        force: false,
    };

    let result = evaluate_registration(&request, &config, &state, Instant::now())
        .expect("evaluate_registration must not error");

    // Then: request is rejected
    assert!(
        matches!(result, RegistrationOutcome::WorkflowQuarantined { .. }),
        "expected WorkflowQuarantined rejection, got {result:?}"
    );

    // And: no active version changes — the workflow remains Quarantined
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Quarantined,
        "quarantine status must be unchanged after rejected deployment"
    );
}

// ─── BDD: Active workflow allows deployment (control) ────────────────────────

#[test]
fn given_active_workflow_when_automated_deploy_arrives_then_deploy_is_allowed() {
    // Given: workflow W is Active
    let config = CircuitBreakerConfig::default_config().unwrap();
    let state = Arc::new(CircuitBreakerState::new());
    let wf = make_name("healthy-wf");

    // When: automated deployment request arrives
    let request = RegistrationRequest {
        workflow_name: wf.clone(),
        binary_hash: make_hash("aaaa1111bbbb2222"),
        force: false,
    };

    let result = evaluate_registration(&request, &config, &state, Instant::now())
        .expect("evaluate_registration must not error");

    // Then: deployment is allowed
    assert_eq!(
        result,
        RegistrationOutcome::Allowed,
        "active workflow must allow automated deployment"
    );
}

// ─── BDD: Force flag bypasses quarantine gate ────────────────────────────────

#[test]
fn given_quarantined_workflow_when_force_deploy_arrives_then_deploy_is_allowed() {
    // Given: workflow W is Quarantined
    let config = CircuitBreakerConfig::default_config().unwrap();
    let state = Arc::new(CircuitBreakerState::new());
    let wf = make_name("force-deploy-wf");
    state.set_status(wf.clone(), RegistrationStatus::Quarantined);

    // When: deployment request arrives with force=true (human operator override)
    let request = RegistrationRequest {
        workflow_name: wf.clone(),
        binary_hash: make_hash("aaaa1111bbbb2222"),
        force: true,
    };

    let result = evaluate_registration(&request, &config, &state, Instant::now())
        .expect("evaluate_registration must not error");

    // Then: force deployment bypasses quarantine gate (ADR-026, POST-005)
    assert_eq!(
        result,
        RegistrationOutcome::Allowed,
        "force flag must bypass quarantine gate"
    );
}

// ─── BDD: Quarantine survives multiple deployment attempts ───────────────────

#[test]
fn given_quarantined_workflow_when_multiple_deploys_arrive_then_all_are_rejected() {
    // Given: workflow W is Quarantined
    let config = CircuitBreakerConfig::default_config().unwrap();
    let state = Arc::new(CircuitBreakerState::new());
    let wf = make_name("persistent-quarantine");
    state.set_status(wf.clone(), RegistrationStatus::Quarantined);

    // When: multiple automated deployment attempts arrive with different hashes
    for i in 0..5 {
        let request = RegistrationRequest {
            workflow_name: wf.clone(),
            binary_hash: make_hash(&format!(
                "{i:04x}000000000000000000000000000000000000000000000000000000000000"
            )),
            force: false,
        };

        let result = evaluate_registration(&request, &config, &state, Instant::now())
            .expect("evaluate_registration must not error");

        // Then: each attempt is rejected
        assert!(
            matches!(result, RegistrationOutcome::WorkflowQuarantined { .. }),
            "attempt {i}: expected WorkflowQuarantined, got {result:?}"
        );
    }

    // And: quarantine persists after all rejected attempts
    assert_eq!(
        state.get_status(&wf),
        RegistrationStatus::Quarantined,
        "quarantine must persist across multiple rejected deployment attempts"
    );
}
