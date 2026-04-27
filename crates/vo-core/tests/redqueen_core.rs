#![allow(clippy::redundant_pattern_matching)]
//! RED-QUEEN coevolutionary adversarial tests for vo-core engine subsystems.
//!
//! Attack surfaces: state transitions, workload budget mutations, effect lifecycle,
//! replay engine sequence integrity, and admission control boundary conditions.

use vo_core::effects::{can_commit, can_rollback, commit_effect, is_terminal, rollback_effect};
use vo_core::replay::{ReplayEngine, ReplayError};
use vo_core::workload_class::{DegradedBudget, WorkloadBudget, WorkloadClass};
use vo_types::state::{apply, LifecycleState, TransitionEvent};
use vo_types::{EffectIntent, EffectKind, TimestampMs};

fn prepared_effect(id: &str) -> vo_types::EffectRecord {
    vo_types::EffectRecord::new(
        id.to_string(),
        EffectKind::HttpCall,
        serde_json::json!({}).into(),
        EffectIntent::Prepared,
        None,
    )
    .expect("valid effect")
}

fn committed_effect(id: &str) -> vo_types::EffectRecord {
    vo_types::EffectRecord::new(
        id.to_string(),
        EffectKind::HttpCall,
        serde_json::json!({}).into(),
        EffectIntent::Committed,
        Some(TimestampMs::new_unchecked(9999)),
    )
    .expect("valid effect")
}

fn make_envelope(instance: &str, seq: u64) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1, "instance_id": instance, "sequence": seq,
        "timestamp_ms": 1000 * seq,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf",
            "binary_hash": "h", "workflow_version_hash": "wv",
            "dedupe_key_hash": null, "version": 1},
        "metadata": {}
    })
}

// ATTACK VECTOR 1: State machine — invalid transitions must always reject

#[test]
fn rq_core_01_terminal_states_reject_all_transitions() {
    let terminals = [LifecycleState::Completed, LifecycleState::Cancelled];
    let events = [
        TransitionEvent::AssignToNode,
        TransitionEvent::StepScheduled,
        TransitionEvent::ExecuteStep,
        TransitionEvent::CompleteStep,
        TransitionEvent::WaitForTimer,
        TransitionEvent::TimerFired,
    ];
    for state in &terminals {
        for event in &events {
            assert!(
                apply(*state, *event).is_err(),
                "{state:?} + {event:?} should fail"
            );
        }
    }
}

#[test]
fn rq_core_02_cancel_from_eligible_nonterminal_states() {
    let eligible = [
        LifecycleState::Pending,
        LifecycleState::RunningDecision,
        LifecycleState::StepScheduled,
        LifecycleState::StepExecuting,
        LifecycleState::WaitingForTimer,
    ];
    for state in &eligible {
        assert_eq!(
            apply(*state, TransitionEvent::Cancel),
            Ok(LifecycleState::Cancelled)
        );
    }
    assert!(apply(LifecycleState::PendingPublication, TransitionEvent::Cancel).is_err());
}

#[test]
fn rq_core_03_failed_only_allows_resume() {
    let bad = [
        TransitionEvent::AssignToNode,
        TransitionEvent::StepScheduled,
        TransitionEvent::ExecuteStep,
        TransitionEvent::CompleteStep,
        TransitionEvent::WaitForTimer,
    ];
    for event in &bad {
        assert!(apply(LifecycleState::Failed, *event).is_err());
    }
    assert_eq!(
        apply(LifecycleState::Failed, TransitionEvent::InstanceResumed),
        Ok(LifecycleState::RunningDecision),
    );
}

// ATTACK VECTOR 2: Effect lifecycle — terminal immutability

#[test]
fn rq_core_04_committed_effect_rejects_recommit_and_rollback() {
    let e = committed_effect("eff-1");
    assert!(is_terminal(&e));
    assert!(!can_commit(&e));
    assert!(!can_rollback(&e));
    assert!(commit_effect(&e, TimestampMs::new_unchecked(1)).is_err());
    assert!(rollback_effect(&e).is_err());
}

#[test]
fn rq_core_05_prepared_accepts_both_transitions() {
    let e = prepared_effect("eff-2");
    assert!(!is_terminal(&e));
    assert!(can_commit(&e));
    assert!(can_rollback(&e));
    let c = commit_effect(&e, TimestampMs::new_unchecked(42)).expect("commit");
    assert!(is_terminal(&c));
    let rolled = rollback_effect(&prepared_effect("eff-3")).expect("rollback");
    assert!(is_terminal(&rolled));
}

#[test]
fn rq_core_06_double_transition_fails() {
    let e = prepared_effect("eff-4");
    let c = commit_effect(&e, TimestampMs::new_unchecked(1)).unwrap();
    assert!(commit_effect(&c, TimestampMs::new_unchecked(2)).is_err());
    assert!(rollback_effect(&c).is_err());
}

// ATTACK VECTOR 3: WorkloadBudget — acquire/release under pressure

#[test]
fn rq_core_07_budget_exhaustion_rejects_acquires() {
    let budget = WorkloadBudget::new(1, 0, 0, 0);
    assert!(budget.can_acquire(WorkloadClass::ExactCritical));
    budget.acquire(WorkloadClass::ExactCritical).unwrap();
    assert!(!budget.can_acquire(WorkloadClass::ExactCritical));
    assert!(budget.acquire(WorkloadClass::ExactCritical).is_err());
}

#[test]
fn rq_core_08_release_restores_capacity() {
    let budget = WorkloadBudget::new(1, 0, 0, 0);
    budget.acquire(WorkloadClass::ExactCritical).unwrap();
    budget.release(WorkloadClass::ExactCritical);
    assert!(budget.can_acquire(WorkloadClass::ExactCritical));
}

#[test]
fn rq_core_09_classes_isolated_no_cross_contamination() {
    let budget = WorkloadBudget::new(1, 1, 1, 1);
    budget.acquire(WorkloadClass::ExactCritical).unwrap();
    assert!(budget.can_acquire(WorkloadClass::Standard));
    assert!(budget.can_acquire(WorkloadClass::Recovery));
}

#[test]
fn rq_core_10_degraded_mode_blocks_non_critical_classes() {
    let mut budget = DegradedBudget::new(1, 4, 2, 8);
    assert!(!budget.is_degraded());
    budget.enter_degraded();
    assert!(budget.is_degraded());
    assert!(!budget.can_acquire(WorkloadClass::Standard));
    assert!(budget.acquire(WorkloadClass::Standard).is_err());
    assert!(budget.can_acquire(WorkloadClass::ExactCritical));
    budget.exit_degraded();
    assert!(budget.can_acquire(WorkloadClass::Standard));
}

// ATTACK VECTOR 4: Replay engine — sequence corruption detection

#[test]
fn rq_core_11_replay_sequence_gap_detected() {
    let engine = ReplayEngine::new();
    let events: Vec<_> = [1u64, 2, 5]
        .iter()
        .map(|s| serde_json::from_value(make_envelope("inst", *s)).unwrap())
        .collect();
    let err = engine.replay(&events).unwrap_err();
    assert!(matches!(
        err,
        ReplayError::SequenceGap {
            expected: 3,
            actual: 5,
            ..
        }
    ));
}

#[test]
fn rq_core_12_replay_duplicate_sequence_detected() {
    let engine = ReplayEngine::new();
    let events: Vec<_> = [1u64, 2, 2]
        .iter()
        .map(|s| serde_json::from_value(make_envelope("inst", *s)).unwrap())
        .collect();
    let err = engine.replay(&events).unwrap_err();
    assert!(matches!(
        err,
        ReplayError::SequenceDuplicate { sequence: 2, .. }
    ));
}

#[test]
fn rq_core_13_replay_instance_mismatch_detected() {
    let engine = ReplayEngine::new();
    let events = vec![
        serde_json::from_value(make_envelope("aaa", 1)).unwrap(),
        serde_json::from_value(make_envelope("bbb", 2)).unwrap(),
    ];
    let err = engine.replay(&events).unwrap_err();
    assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
}

// ATTACK VECTOR 5: Budget integrity — acquire/release balance

#[test]
fn rq_core_14_budget_acquire_release_cycle_maintains_balance() {
    let budget = WorkloadBudget::new(2, 3, 1, 0);
    for _ in 0..3 {
        budget.acquire(WorkloadClass::Standard).unwrap();
    }
    assert!(!budget.can_acquire(WorkloadClass::Standard));
    assert_eq!(budget.remaining(WorkloadClass::Standard), 0);
    budget.release(WorkloadClass::Standard);
    assert!(budget.can_acquire(WorkloadClass::Standard));
    assert_eq!(budget.remaining(WorkloadClass::Standard), 1);
    assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 2);
    assert_eq!(budget.remaining(WorkloadClass::Recovery), 1);
}
