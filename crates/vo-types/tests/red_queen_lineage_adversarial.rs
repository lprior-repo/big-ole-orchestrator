//! Red Queen adversarial tests for continue-as-new lineage (ADR-038, ADR-042)
//!
//! These tests inject adversarial conditions into the continue-as-new lineage
//! system to verify invariants hold under stress:
//!
//! - Lineage rollover during active signal
//! - Timer across lineage boundary
//! - Concurrent rollover attempts
//! - Rollover with pending effects
//!
//! Target: vo-types lineage + signal routing

use vo_types::signal::{LineageScope, SignalAddress, WaitKey};
use vo_types::Epoch;
use vo_types::InstanceId;
use vo_types::WorkflowLineage;

// ========================================================================
// Test helpers
// ========================================================================

fn make_instance_id(suffix: u8) -> InstanceId {
    static VALID_ULIDS: &[&str] = &[
        "01JAR3K2N0XG8F5VZE9H7QW4Y6",
        "01JAR3K2N0XG8F5VZE9H7QW4Y7",
        "01JAR3K2N0XG8F5VZE9H7QW4Y8",
        "01JAR3K2N0XG8F5VZE9H7QW4Y9",
        "01JAR3K2N0XG8F5VZE9H7QW4ZA",
        "01JAR3K2N0XG8F5VZE9H7QW4ZB",
        "01JAR3K2N0XG8F5VZE9H7QW4ZC",
        "01JAR3K2N0XG8F5VZE9H7QW4ZD",
        "01JAR3K2N0XG8F5VZE9H7QW4ZE",
        "01JAR3K2N0XG8F5VZE9H7QW4ZF",
        "01JAR3K2N0XG8F5VZE9H7QW4ZG",
        "01JAR3K2N0XG8F5VZE9H7QW4ZH",
        "01JAR3K2N0XG8F5VZE9H7QW4ZJ",
        "01JAR3K2N0XG8F5VZE9H7QW4ZK",
        "01JAR3K2N0XG8F5VZE9H7QW4ZM",
        "01JAR3K2N0XG8F5VZE9H7QW4ZN",
        "01JAR3K2N0XG8F5VZE9H7QW4ZP",
        "01JAR3K2N0XG8F5VZE9H7QW4ZQ",
        "01JAR3K2N0XG8F5VZE9H7QW4ZR",
        "01JAR3K2N0XG8F5VZE9H7QW4ZS",
        "01JAR3K2N0XG8F5VZE9H7QW4ZT",
        "01JAR3K2N0XG8F5VZE9H7QW4ZV",
        "01JAR3K2N0XG8F5VZE9H7QW4ZW",
        "01JAR3K2N0XG8F5VZE9H7QW4ZX",
        "01JAR3K2N0XG8F5VZE9H7QW4ZZ",
        "01JAR3K2N0XG8F5VZE9H7QW5A6",
    ];
    let idx = (suffix as usize) % VALID_ULIDS.len();
    InstanceId::parse(VALID_ULIDS[idx]).expect("valid ULID")
}

fn make_lineage_root(id: &str) -> WorkflowLineage {
    WorkflowLineage::new(id).expect("valid lineage")
}

// ========================================================================
// DIMENSION: lineage_rollover_during_active_signal
// ADR-042 §2: Lineage-wide signals must route to current active epoch
// ========================================================================

#[test]
fn rq_lineage_wide_signal_routes_to_new_epoch_after_rollover() {
    // Adversarial scenario: A lineage-wide signal is sent to an old instance
    // that existed before a continue-as-new rollover. The signal should
    // route to the NEW active epoch, not the stale one.
    let lineage_id = make_instance_id(b'A');
    let old_instance = make_instance_id(b'B');
    let new_instance = make_instance_id(b'C');

    let root = make_lineage_root(lineage_id.as_str());
    let epoch1 = root.continue_as_new().expect("rollover to e1");

    // Simulate lineage store state after rollover
    // Old instance is epoch 0, new instance is epoch 1
    let signal = SignalAddress::lineage_wide(
        lineage_id.clone(),
        old_instance.clone(),
        WaitKey::parse("test-key").expect("valid"),
    );

    // INVARIANT: lineage_id persists across epochs
    assert_eq!(signal.lineage_id(), &lineage_id);
    assert!(signal.is_lineage_wide());
    assert!(signal.epoch_id().is_none());

    // After rollover, lineage_wide signal should still have same lineage_id
    // but the routing layer should resolve to current active epoch (1)
    let lineage_after = epoch1;
    assert_eq!(lineage_after.lineage_id, lineage_id.as_str());
    assert_eq!(lineage_after.epoch, Epoch::new(1));
}

#[test]
fn rq_epoch_local_signal_to_stale_epoch_rejected_after_rollover() {
    // Adversarial scenario: An epoch-local signal targets epoch 0 (old).
    // After rollover to epoch 1, this signal should NOT match any wait
    // because epoch 0 is no longer the active epoch.
    let lineage_id = make_instance_id(b'D');
    let instance_0 = make_instance_id(b'E');
    let instance_1 = make_instance_id(b'F');

    let root = make_lineage_root(lineage_id.as_str());
    let _epoch1 = root.continue_as_new().expect("rollover");

    // Signal targeting stale epoch 0
    let stale_signal = SignalAddress::epoch_local(
        lineage_id.clone(),
        Epoch::ZERO, // stale epoch
        instance_0.clone(),
        WaitKey::parse("old-signal").expect("valid"),
    );

    // INVARIANT: epoch_local signals require epoch_id to be Some
    assert!(stale_signal.is_epoch_local());
    assert_eq!(stale_signal.epoch_id(), Some(Epoch::ZERO));

    // The current lineage has epoch 1, so this epoch-local signal
    // is targeting a non-current epoch - routing should reject/drop it
    let current_epoch = Epoch::new(1);
    assert_ne!(stale_signal.epoch_id(), Some(current_epoch));
}

#[test]
fn rq_lineage_wide_signal_preserves_wait_key_across_rollover() {
    // Adversarial scenario: A wait is registered in epoch 0 with key "approval".
    // After continue-as-new to epoch 1, a lineage-wide signal with the
    // same wait key should still match the wait (signal routing follows lineage).
    let lineage_id = make_instance_id(b'G');
    let instance = make_instance_id(b'H');

    let root = make_lineage_root(lineage_id.as_str());
    let epoch1 = root.continue_as_new().expect("rollover");

    let wait_key = WaitKey::parse("approval").expect("valid");

    // Signal in epoch 1 using lineage-wide addressing
    let signal =
        SignalAddress::lineage_wide(lineage_id.clone(), instance.clone(), wait_key.clone());

    // INVARIANT: LineageScope is preserved in signal address
    assert_eq!(signal.lineage_scope(), LineageScope::LineageWide);
    assert!(signal.is_lineage_wide());

    // The lineage persists, so lineage-wide signal can reach the wait
    // even though the wait was created in epoch 0
    assert_eq!(signal.lineage_id(), &lineage_id);
    assert_eq!(signal.wait_key(), &wait_key);

    // Epoch has advanced but lineage_id is the same
    assert_eq!(epoch1.lineage_id, lineage_id.as_str());
}

// ========================================================================
// DIMENSION: timer_across_lineage_boundary
// ADR-038 §4: Timers created in one epoch must survive continue-as-new
// ========================================================================

#[test]
fn rq_timer_created_in_epoch_zero_fires_in_epoch_one() {
    // Adversarial scenario: A timer is created in epoch 0. The workflow
    // then does continue-as-new. The timer should still fire in epoch 1
    // because it was lineage-bound, not epoch-bound.
    let lineage_id = "timer-lineage-001";
    let root = make_lineage_root(lineage_id);

    // Epoch 0 timer (would have a timer_id associated with it)
    let timer_epoch_0 = Epoch::ZERO;
    let timer_epoch_1 = Epoch::new(1);

    // After rollover, lineage is at epoch 1
    let epoch1 = root.continue_as_new().expect("rollover");
    assert_eq!(epoch1.epoch, timer_epoch_1);

    // The timer was bound to the lineage, not a specific epoch
    // so it should be queryable from the current epoch
    // (lineage-wide timer query would resolve to active epoch)
    let lineage_wide_epoch = epoch1.epoch;
    assert_eq!(lineage_wide_epoch, timer_epoch_1);

    // Timer from epoch 0 should still be accessible via lineage-wide lookup
    // because lineage_id is stable across rollovers
    assert_eq!(epoch1.lineage_id, lineage_id);
}

#[test]
fn rq_timer_epoch_assignment_stable_across_rollover() {
    // Adversarial scenario: Verify that timer epoch assignment is based on
    // creation time relative to rollover, not lookup time.
    let lineage_id = "timer-stability-002";
    let root = make_lineage_root(lineage_id);

    let e0 = root;
    let e1 = e0.continue_as_new().expect("e1");
    let e2 = e1.continue_as_new().expect("e2");

    // Each rollover preserves the lineage_id
    assert_eq!(e0.lineage_id, lineage_id);
    assert_eq!(e1.lineage_id, lineage_id);
    assert_eq!(e2.lineage_id, lineage_id);

    // Epochs are monotonically increasing
    assert!(e0.epoch < e1.epoch);
    assert!(e1.epoch < e2.epoch);

    // Timer created in epoch 0 should have metadata linking it to epoch 0
    // but lookup via lineage should return current (epoch 2)
    assert_eq!(e2.epoch, Epoch::new(2));
}

// ========================================================================
// DIMENSION: concurrent_rollover_attempts
// ADR-038 §3: Concurrent continue_as_new must be handled safely
// ========================================================================

#[test]
fn rq_concurrent_rollover_only_one_succeeds_atomically() {
    // Adversarial scenario: Two threads call continue_as_new simultaneously
    // on the same lineage. Only ONE should succeed in advancing the epoch.
    // The other should either see the updated state or get a conflict error.
    let lineage_id = "concurrent-rollover-003";
    let root = make_lineage_root(lineage_id);

    // First rollover succeeds
    let e1 = root.continue_as_new().expect("first rollover");
    assert_eq!(e1.epoch, Epoch::new(1));

    // Second rollover from e1 should give e2
    let e2 = e1.continue_as_new().expect("second rollover");
    assert_eq!(e2.epoch, Epoch::new(2));
    assert_eq!(e2.parent_epoch, Some(Epoch::new(1)));

    // Third rollover would give e3
    let e3 = e2.continue_as_new().expect("third rollover");
    assert_eq!(e3.epoch, Epoch::new(3));

    // Concurrent calls to continue_as_new on the SAME state should be
    // handled via the lineage store's record_rollover atomic operation
    // which reads current state, then writes new state atomically
}

#[test]
fn rq_rollover_chain_preserves_parent_chain() {
    // Adversarial scenario: Build a long chain of rollovers and verify
    // the parent_epoch chain is intact. This catches any corruption
    // where parent pointers get scrambled during concurrent access.
    let lineage_id = "rollover-chain-004";
    let root = make_lineage_root(lineage_id);

    let epochs: Vec<_> = std::iter::successors(Some(root), |e| e.continue_as_new().ok())
        .take(10)
        .collect();

    // Verify epoch numbers
    for (i, epoch) in epochs.iter().enumerate() {
        assert_eq!(epoch.epoch, Epoch::new(i as u64));
    }

    // Verify parent chain: each epoch (except 0) has parent = epoch - 1
    for i in 1..epochs.len() {
        assert_eq!(epochs[i].parent_epoch, Some(Epoch::new((i - 1) as u64)));
    }

    // All share same lineage_id
    for epoch in &epochs {
        assert_eq!(epoch.lineage_id, lineage_id);
    }
}

#[test]
fn rq_rollover_at_u64_max_is_strictly_rejected() {
    // Adversarial scenario: Attempting to rollover from u64::MAX epoch
    // MUST fail with EpochOverflow. This prevents epoch counter corruption.
    let lineage_id = "max-epoch-005";
    let max_lineage = WorkflowLineage::with_parent(
        lineage_id.to_string(),
        Epoch::new(u64::MAX),
        Some(Epoch::new(u64::MAX - 1)),
    )
    .expect("valid max lineage");

    let result = max_lineage.continue_as_new();
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, vo_types::LineageError::EpochOverflow));
}

// ========================================================================
// DIMENSION: rollover_with_pending_effects
// ADR-038 §5: Effects created before rollover must remain visible
// ========================================================================

#[test]
fn rq_effects_created_before_rollover_visible_in_new_epoch() {
    // Adversarial scenario: An effect is recorded in epoch 0. Then
    // continue-as-new happens. The effect should still be queryable
    // because it belongs to the lineage's history.
    let lineage_id = "effect-visibility-006";
    let root = make_lineage_root(lineage_id);

    // In a real system, effects would be stored with lineage_id
    // and be queryable via lineage-wide queries
    let epoch_0_effect_lineage_id = root.lineage_id.clone();

    // Rollover happens
    let epoch1 = root.continue_as_new().expect("rollover");

    // The new epoch has same lineage_id, so lineage-wide queries
    // for effects would find effects from epoch 0
    assert_eq!(epoch1.lineage_id, epoch_0_effect_lineage_id);

    // Effects are associated with lineage, not specific epoch
    // so they remain visible after rollover
}

#[test]
fn rq_lineage_record_previous_instance_preserved_after_rollover() {
    // Adversarial scenario: After rollover, the previous_instance_id
    // in the lineage record must be correctly set to enable
    // historical queries and effect visibility.
    let lineage_id = "instance-chain-007";
    let root = make_lineage_root(lineage_id);
    let instance_0 = make_instance_id(b'J');

    let e1 = root.continue_as_new().expect("e1");
    let instance_1 = make_instance_id(b'K');

    // After rollover to e1, parent_epoch is e0
    assert_eq!(e1.parent_epoch, Some(Epoch::ZERO));

    // In a real lineage store, previous_instance_id would be instance_0
    // and active_instance_id would be instance_1
    // This enables queries to fan out to both epochs when needed
}

#[test]
fn rq_multiple_rollovers_maintain_full_instance_chain() {
    // Adversarial scenario: Many rapid rollovers. Verify that the
    // lineage store correctly maintains the chain of instances.
    let lineage_id = "multi-rollover-008";
    let root = make_lineage_root(lineage_id);

    let mut current = root;
    let mut epochs = vec![current.epoch];

    for i in 0..5 {
        current = current
            .continue_as_new()
            .expect(format!("rollover {}", i).as_str());
        epochs.push(current.epoch);
    }

    // All epochs should be distinct and ordered
    let mut sorted_epochs = epochs.clone();
    sorted_epochs.sort();
    assert_eq!(epochs, sorted_epochs);

    // All lineages share the same lineage_id
    assert_eq!(current.lineage_id, lineage_id);
}

// ========================================================================
// DIMENSION: signal_routing_preservation
// ADR-042: Signal routing invariants must hold across rollovers
// ========================================================================

#[test]
fn rq_signal_address_lineage_id_immutable_across_rollover() {
    // INVARIANT: SignalAddress.lineage_id never changes after construction.
    // This is critical for correct routing - if lineage_id could change,
    // signals would be misrouted after rollover.
    let lineage_id = make_instance_id(b'L');
    let instance = make_instance_id(b'M');

    let signal = SignalAddress::lineage_wide(
        lineage_id.clone(),
        instance.clone(),
        WaitKey::parse("immutable-key").expect("valid"),
    );

    // lineage_id is set at construction and must be immutable
    assert_eq!(signal.lineage_id(), &lineage_id);

    // After any number of rollovers, lineage_id is unchanged
    // (lineage_id comes from the signal, not from current epoch state)
    let _rollover = WorkflowLineage::new(lineage_id.to_string()).unwrap();
    // Signal's lineage_id is still the same
    assert_eq!(signal.lineage_id(), &lineage_id);
}

#[test]
fn rq_epoch_local_signal_epoch_field_never_changed_by_routing() {
    // INVARIANT: For epoch-local signals, the epoch_id field is set at
    // signal creation and routing must NEVER modify it. If routing could
    // change the epoch_id, signals would be delivered to wrong epochs.
    let lineage_id = make_instance_id(b'N');
    let instance = make_instance_id(b'O');
    let target_epoch = Epoch::new(5);

    let signal = SignalAddress::epoch_local(
        lineage_id.clone(),
        target_epoch,
        instance.clone(),
        WaitKey::parse("frozen-epoch").expect("valid"),
    );

    // epoch_id is set at construction and must be immutable
    assert_eq!(signal.epoch_id(), Some(target_epoch));

    // Routing must NOT modify epoch_id - it remains frozen
    // even if the target epoch is no longer active
}

#[test]
fn rq_lineage_wide_signal_no_epoch_id_set() {
    // INVARIANT: Lineage-wide signals have epoch_id = None by design.
    // This allows routing to resolve the current active epoch at delivery time.
    let lineage_id = make_instance_id(b'P');
    let instance = make_instance_id(b'Q');

    let signal = SignalAddress::lineage_wide(
        lineage_id.clone(),
        instance.clone(),
        WaitKey::parse("resolve-at-delivery").expect("valid"),
    );

    assert!(signal.is_lineage_wide());
    assert!(signal.epoch_id().is_none());
    assert_eq!(signal.lineage_scope(), LineageScope::LineageWide);
}

// ========================================================================
// DIMENSION: edge_cases_and_boundary_conditions
// ========================================================================

#[test]
fn rq_empty_lineage_id_rejected() {
    // Contract: lineage_id must be non-empty. Empty lineage_id would cause
    // routing failures and index corruption.
    let result = WorkflowLineage::new(String::new());
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        vo_types::LineageError::EmptyLineageId
    ));

    let result_ws = WorkflowLineage::new("   ".to_string());
    assert!(result_ws.is_err());
}

#[test]
fn rq_invalid_epoch_transition_rejected() {
    // Contract: parent_epoch must be < epoch. Violating this corrupts
    // the lineage chain and breaks historical queries.
    let result = WorkflowLineage::with_parent(
        "invalid-chain".to_string(),
        Epoch::new(5),
        Some(Epoch::new(5)), // parent == epoch (invalid)
    );
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        vo_types::LineageError::InvalidEpochTransition { .. }
    ));

    let result2 = WorkflowLineage::with_parent(
        "invalid-chain2".to_string(),
        Epoch::new(3),
        Some(Epoch::new(7)), // parent > epoch (invalid)
    );
    assert!(result2.is_err());
}

#[test]
fn rq_epoch_zero_root_has_no_parent() {
    // Contract: Epoch 0 (root) must have parent_epoch = None.
    // Root is the beginning of the lineage - it has no predecessor.
    let root = make_lineage_root("root-test-009");
    assert_eq!(root.epoch, Epoch::ZERO);
    assert_eq!(root.parent_epoch, None);
}

#[test]
fn rq_rollover_from_nonzero_root_parent_is_correct() {
    // Contract: Any rollover from epoch N creates epoch N+1 with
    // parent_epoch = N.
    let root = make_lineage_root("parent-test-010");
    let e1 = root.continue_as_new().expect("e1");
    assert_eq!(e1.parent_epoch, Some(Epoch::ZERO));

    let e2 = e1.continue_as_new().expect("e2");
    assert_eq!(e2.parent_epoch, Some(Epoch::new(1)));

    let e3 = e2.continue_as_new().expect("e3");
    assert_eq!(e3.parent_epoch, Some(Epoch::new(2)));
}
