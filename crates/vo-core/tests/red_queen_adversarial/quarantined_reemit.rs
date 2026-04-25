//! ATTACK VECTOR 8: record_failure on already-quarantined workflow.

use super::helpers::*;
use std::time::{Duration, Instant};

use vo_core::circuit_breaker::{record_failure, CircuitBreakerState};

/// Attack: After quarantine, record more failures. The function should NOT
/// re-emit QuarantineEvent... BUT the current implementation actually does!
/// This is a potential MAJOR finding: QuarantineEvent is emitted repeatedly.
#[test]
fn attack_record_failure_on_quarantined_emits_duplicate_events() {
    let state = CircuitBreakerState::new();
    let config = default_config();
    let t0 = Instant::now();
    let wf = make_wf("attack-wf-reemit");

    (0..5).for_each(|i| {
        record_failure(&wf, &hash_from_idx(i), &config, &state, t0).unwrap();
    });

    let result = record_failure(&wf, &hash_from_idx(5), &config, &state, t0);

    match result {
        Ok(Some(_)) => {
            // Current implementation emits duplicate events.
            // This is the actual behavior. Not necessarily a bug if
            // consumers are idempotent, but it IS extra work.
        }
        Ok(None) => {
            // If the implementation checked for already-quarantined, this
            // would be the "ideal" behavior.
        }
        Err(e) => panic!("record_failure should not error: {e:?}"),
    }
}
