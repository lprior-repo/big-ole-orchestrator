//! Shared helpers for Red Queen adversarial tests.
//!
//! These functions are reused across all attack vector test modules.

use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, unquarantine, CircuitBreakerConfig,
    CircuitBreakerState, RegistrationOutcome, RegistrationRequest,
};
use vo_types::{BinaryHash, WorkflowName};

pub fn make_wf(s: &str) -> WorkflowName {
    WorkflowName::parse(s).expect("test workflow name should be valid")
}

pub fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).expect("test hash should be valid")
}

pub fn hash_from_idx(i: usize) -> BinaryHash {
    let hex = format!("{i:08x}");
    BinaryHash::parse(&hex).expect("generated hash should be valid")
}

pub fn default_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig::new(Duration::from_secs(60), Duration::from_secs(600), 5)
        .expect("default config should be valid")
}

pub fn make_request(wf: &str, hash: &str, force: bool) -> RegistrationRequest {
    RegistrationRequest {
        workflow_name: make_wf(wf),
        binary_hash: make_hash(hash),
        force,
    }
}
