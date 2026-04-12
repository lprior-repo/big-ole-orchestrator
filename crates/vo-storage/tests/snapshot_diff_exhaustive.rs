use vo_storage::snapshot_diff::{
    apply_diff, diff, invert_diff, ApplyError, DiffError, DiffOperation, DiffResult, SnapshotDiff,
    StateDiff,
};
use vo_types::state::InstanceState;
use vo_types::InstanceId;

fn test_id(bytes: u8) -> InstanceId {
    InstanceId::from_bytes([bytes; 16])
}

// ═══════════════════════════════════════════════════════════════
// B-001: diff() returns Identical when sequences are equal
// ═══════════════════════════════════════════════════════════════

#[test]
fn b001_diff_returns_identical_when_sequences_equal() {
    let result = diff(
        test_id(1),
        &(5, InstanceState { counter: 10 }),
        &(5, InstanceState { counter: 20 }),
    );
    assert!(matches!(result, DiffResult::Identical));
}

#[test]
fn b001_diff_returns_identical_when_sequences_equal_and_states_equal() {
    let state = InstanceState { counter: 42 };
    let result = diff(test_id(1), &(3, state.clone()), &(3, state));
    assert!(matches!(result, DiffResult::Identical));
}

// ═══════════════════════════════════════════════════════════════
// B-002: diff() returns Identical on sequence regression
// ═══════════════════════════════════════════════════════════════

#[test]
fn b002_diff_returns_identical_when_to_sequence_less_than_from() {
    let result = diff(
        test_id(1),
        &(10, InstanceState { counter: 10 }),
        &(5, InstanceState { counter: 20 }),
    );
    assert!(matches!(result, DiffResult::Identical));
}

#[test]
fn b002_diff_returns_identical_when_to_sequence_zero_and_from_nonzero() {
    let result = diff(
        test_id(1),
        &(1, InstanceState { counter: 10 }),
        &(0, InstanceState { counter: 20 }),
    );
    assert!(matches!(result, DiffResult::Identical));
}

// ═══════════════════════════════════════════════════════════════
// B-003: diff() returns Unchanged when counters equal
// ═══════════════════════════════════════════════════════════════

#[test]
fn b003_diff_returns_unchanged_when_counters_equal() {
    let state = InstanceState { counter: 42 };
    let result = diff(test_id(1), &(1, state.clone()), &(5, state));
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Unchanged));
        }
        _ => panic!("Expected HasDiff with Unchanged"),
    }
}

#[test]
fn b003_diff_returns_unchanged_when_both_counters_zero() {
    let result = diff(
        test_id(1),
        &(1, InstanceState { counter: 0 }),
        &(2, InstanceState { counter: 0 }),
    );
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Unchanged));
        }
        _ => panic!("Expected HasDiff with Unchanged"),
    }
}

// ═══════════════════════════════════════════════════════════════
// B-004: diff() returns Added when from is zero
// ═══════════════════════════════════════════════════════════════

#[test]
fn b004_diff_returns_added_when_from_zero_to_positive() {
    let result = diff(
        test_id(1),
        &(0, InstanceState { counter: 0 }),
        &(1, InstanceState { counter: 100 }),
    );
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Added(100)));
        }
        _ => panic!("Expected HasDiff with Added(100)"),
    }
}

#[test]
fn b004_diff_returns_added_with_large_value() {
    let result = diff(
        test_id(1),
        &(0, InstanceState { counter: 0 }),
        &(1, InstanceState { counter: u64::MAX }),
    );
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Added(u64::MAX)));
        }
        _ => panic!("Expected HasDiff with Added(u64::MAX)"),
    }
}

// ═══════════════════════════════════════════════════════════════
// B-005: diff() returns Removed when to is zero
// ═══════════════════════════════════════════════════════════════

#[test]
fn b005_diff_returns_removed_when_from_positive_to_zero() {
    let result = diff(
        test_id(1),
        &(1, InstanceState { counter: 50 }),
        &(2, InstanceState { counter: 0 }),
    );
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Removed(50)));
        }
        _ => panic!("Expected HasDiff with Removed(50)"),
    }
}

#[test]
fn b005_diff_returns_removed_with_large_value() {
    let result = diff(
        test_id(1),
        &(1, InstanceState { counter: u64::MAX }),
        &(2, InstanceState { counter: 0 }),
    );
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Removed(u64::MAX)));
        }
        _ => panic!("Expected HasDiff with Removed(u64::MAX)"),
    }
}

// ═══════════════════════════════════════════════════════════════
// B-006: diff() returns Modified when both counters differ and non-zero
// ═══════════════════════════════════════════════════════════════

#[test]
fn b006_diff_returns_modified_when_both_nonzero_and_different() {
    let result = diff(
        test_id(1),
        &(0, InstanceState { counter: 10 }),
        &(1, InstanceState { counter: 20 }),
    );
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Modified(10, 20)));
        }
        _ => panic!("Expected HasDiff with Modified(10, 20)"),
    }
}

#[test]
fn b006_diff_returns_modified_with_decreasing_value() {
    let result = diff(
        test_id(1),
        &(0, InstanceState { counter: 200 }),
        &(1, InstanceState { counter: 100 }),
    );
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Modified(200, 100)));
        }
        _ => panic!("Expected HasDiff with Modified(200, 100)"),
    }
}

// ═══════════════════════════════════════════════════════════════
// B-007: diff() SnapshotDiff carries correct metadata
// ═══════════════════════════════════════════════════════════════

#[test]
fn b007_diff_snapshot_diff_carries_correct_metadata() {
    let id = test_id(42);
    let result = diff(
        id.clone(),
        &(7, InstanceState { counter: 10 }),
        &(13, InstanceState { counter: 20 }),
    );
    match result {
        DiffResult::HasDiff(d) => {
            assert_eq!(d.from_sequence, 7);
            assert_eq!(d.to_sequence, 13);
            assert_eq!(d.instance_id, id);
        }
        _ => panic!("Expected HasDiff"),
    }
}

// ═══════════════════════════════════════════════════════════════
// B-008: apply_diff() with Unchanged preserves base
// ═══════════════════════════════════════════════════════════════

#[test]
fn b008_apply_diff_unchanged_preserves_base_counter() {
    let base = InstanceState { counter: 42 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: test_id(1),
        state_diff: StateDiff {
            counter: DiffOperation::Unchanged,
        },
    };
    let result = apply_diff(&(0, base.clone()), &d);
    assert_eq!(result.unwrap(), base);
}

// ═══════════════════════════════════════════════════════════════
// B-009: apply_diff() with Added sets counter when base is 0
// ═══════════════════════════════════════════════════════════════

#[test]
fn b009_apply_diff_added_sets_counter_when_base_zero() {
    let base = InstanceState { counter: 0 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: test_id(1),
        state_diff: StateDiff {
            counter: DiffOperation::Added(100),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert_eq!(result.unwrap().counter, 100);
}

// ═══════════════════════════════════════════════════════════════
// B-010: apply_diff() with Added rejects non-zero base
// ═══════════════════════════════════════════════════════════════

#[test]
fn b010_apply_diff_added_rejects_nonzero_base() {
    let base = InstanceState { counter: 50 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: test_id(1),
        state_diff: StateDiff {
            counter: DiffOperation::Added(100),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert!(matches!(result, Err(ApplyError::DiffTargetInvalid)));
}

// ═══════════════════════════════════════════════════════════════
// B-011: apply_diff() with Removed sets counter to 0 when base matches
// ═══════════════════════════════════════════════════════════════

#[test]
fn b011_apply_diff_removed_zeros_counter_when_base_matches() {
    let base = InstanceState { counter: 42 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: test_id(1),
        state_diff: StateDiff {
            counter: DiffOperation::Removed(42),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert_eq!(result.unwrap().counter, 0);
}

// ═══════════════════════════════════════════════════════════════
// B-012: apply_diff() with Removed rejects mismatched base
// ═══════════════════════════════════════════════════════════════

#[test]
fn b012_apply_diff_removed_rejects_mismatched_base() {
    let base = InstanceState { counter: 30 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: test_id(1),
        state_diff: StateDiff {
            counter: DiffOperation::Removed(42),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert!(matches!(result, Err(ApplyError::DiffTargetInvalid)));
}

// ═══════════════════════════════════════════════════════════════
// B-013: apply_diff() with Modified updates counter when base matches
// ═══════════════════════════════════════════════════════════════

#[test]
fn b013_apply_diff_modified_updates_counter_when_base_matches() {
    let base = InstanceState { counter: 10 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: test_id(1),
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert_eq!(result.unwrap().counter, 20);
}

// ═══════════════════════════════════════════════════════════════
// B-014: apply_diff() with Modified rejects mismatched base
// ═══════════════════════════════════════════════════════════════

#[test]
fn b014_apply_diff_modified_rejects_mismatched_base() {
    let base = InstanceState { counter: 30 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: test_id(1),
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert!(matches!(result, Err(ApplyError::DiffTargetInvalid)));
}

// ═══════════════════════════════════════════════════════════════
// B-015: apply_diff() returns BaseStateMismatch on sequence mismatch
// ═══════════════════════════════════════════════════════════════

#[test]
fn b015_apply_diff_returns_base_mismatch_on_sequence_mismatch() {
    let base = InstanceState { counter: 10 };
    let d = SnapshotDiff {
        from_sequence: 99,
        to_sequence: 100,
        instance_id: test_id(1),
        state_diff: StateDiff {
            counter: DiffOperation::Unchanged,
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert!(matches!(result, Err(ApplyError::BaseStateMismatch)));
}

#[test]
fn b015_apply_diff_returns_base_mismatch_when_diff_from_zero_but_base_nonzero_seq() {
    let base = InstanceState { counter: 10 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: test_id(1),
        state_diff: StateDiff {
            counter: DiffOperation::Unchanged,
        },
    };
    let result = apply_diff(&(3, base), &d);
    assert!(matches!(result, Err(ApplyError::BaseStateMismatch)));
}

// ═══════════════════════════════════════════════════════════════
// B-016–B-021: invert_diff()
// ═══════════════════════════════════════════════════════════════

#[test]
fn b016_invert_diff_swaps_added_to_removed() {
    let d = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Added(100) },
    };
    assert!(matches!(invert_diff(&d).state_diff.counter, DiffOperation::Removed(100)));
}

#[test]
fn b017_invert_diff_swaps_removed_to_added() {
    let d = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Removed(100) },
    };
    assert!(matches!(invert_diff(&d).state_diff.counter, DiffOperation::Added(100)));
}

#[test]
fn b018_invert_diff_swaps_modified_order() {
    let d = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(10, 20) },
    };
    assert!(matches!(invert_diff(&d).state_diff.counter, DiffOperation::Modified(20, 10)));
}

#[test]
fn b019_invert_diff_preserves_unchanged() {
    let d = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Unchanged },
    };
    assert!(matches!(invert_diff(&d).state_diff.counter, DiffOperation::Unchanged));
}

#[test]
fn b020_invert_diff_swaps_sequences() {
    let d = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(10, 20) },
    };
    let inverted = invert_diff(&d);
    assert_eq!(inverted.from_sequence, 5);
    assert_eq!(inverted.to_sequence, 0);
}

#[test]
fn b021_invert_diff_preserves_instance_id() {
    let id = test_id(42);
    let d = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: id.clone(),
        state_diff: StateDiff { counter: DiffOperation::Added(10) },
    };
    assert_eq!(invert_diff(&d).instance_id, id);
}

// ═══════════════════════════════════════════════════════════════
// B-022–B-032: compose()
// ═══════════════════════════════════════════════════════════════

#[test]
fn b022_compose_rejects_sequence_gap() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(10, 20) },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 6, to_sequence: 10, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(20, 30) },
    };
    assert!(matches!(diff_ab.compose(&diff_bc), Err(DiffError::SequenceGap)));
}

#[test]
fn b023_compose_rejects_mismatched_instance_ids() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(10, 20) },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: test_id(2),
        state_diff: StateDiff { counter: DiffOperation::Modified(20, 30) },
    };
    assert!(matches!(diff_ab.compose(&diff_bc), Err(DiffError::SequenceGap)));
}

#[test]
fn b024_compose_left_identity_unchanged() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Unchanged },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Added(50) },
    };
    assert!(matches!(diff_ab.compose(&diff_bc).unwrap().state_diff.counter, DiffOperation::Added(50)));
}

#[test]
fn b025_compose_right_identity_unchanged() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Removed(42) },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Unchanged },
    };
    assert!(matches!(diff_ab.compose(&diff_bc).unwrap().state_diff.counter, DiffOperation::Removed(42)));
}

#[test]
fn b026_compose_chains_modified_correctly() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(10, 20) },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(20, 30) },
    };
    assert!(matches!(diff_ab.compose(&diff_bc).unwrap().state_diff.counter, DiffOperation::Modified(10, 30)));
}

#[test]
fn b027_compose_rejects_modified_with_mismatched_middle() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(10, 20) },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(99, 30) },
    };
    assert!(matches!(diff_ab.compose(&diff_bc), Err(DiffError::SequenceGap)));
}

#[test]
fn b028_compose_rejects_added_plus_added() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Added(10) },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Added(20) },
    };
    assert!(matches!(diff_ab.compose(&diff_bc), Err(DiffError::SequenceGap)));
}

#[test]
fn b029_compose_rejects_removed_plus_removed() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Removed(10) },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Removed(20) },
    };
    assert!(matches!(diff_ab.compose(&diff_bc), Err(DiffError::SequenceGap)));
}

#[test]
fn b030_compose_rejects_added_plus_modified() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Added(10) },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(10, 20) },
    };
    assert!(matches!(diff_ab.compose(&diff_bc), Err(DiffError::SequenceGap)));
}

#[test]
fn b030_compose_rejects_modified_plus_added() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(10, 20) },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Added(30) },
    };
    assert!(matches!(diff_ab.compose(&diff_bc), Err(DiffError::SequenceGap)));
}

#[test]
fn b030_compose_rejects_removed_plus_modified() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Removed(10) },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(0, 20) },
    };
    assert!(matches!(diff_ab.compose(&diff_bc), Err(DiffError::SequenceGap)));
}

#[test]
fn b031_compose_result_has_correct_sequences() {
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Modified(10, 20) },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Unchanged },
    };
    let result = diff_ab.compose(&diff_bc).unwrap();
    assert_eq!(result.from_sequence, 0);
    assert_eq!(result.to_sequence, 10);
}

#[test]
fn b032_compose_result_preserves_instance_id() {
    let id = test_id(42);
    let diff_ab = SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: id.clone(),
        state_diff: StateDiff { counter: DiffOperation::Unchanged },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5, to_sequence: 10, instance_id: id.clone(),
        state_diff: StateDiff { counter: DiffOperation::Added(99) },
    };
    assert_eq!(diff_ab.compose(&diff_bc).unwrap().instance_id, id);
}

// ═══════════════════════════════════════════════════════════════
// B-033–B-036: Serde roundtrip tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn b033_diff_operation_serde_roundtrip() {
    let cases: Vec<DiffOperation<u64>> = vec![
        DiffOperation::Unchanged,
        DiffOperation::Added(42),
        DiffOperation::Removed(99),
        DiffOperation::Modified(10, 20),
    ];
    for op in cases {
        let json = serde_json::to_string(&op).unwrap();
        let recovered: DiffOperation<u64> = serde_json::from_str(&json).unwrap();
        assert_eq!(op, recovered, "roundtrip failed for {op:?}");
    }
}

#[test]
fn b034_state_diff_serde_roundtrip() {
    let sd = StateDiff { counter: DiffOperation::Modified(5, 10) };
    let json = serde_json::to_string(&sd).unwrap();
    let recovered: StateDiff = serde_json::from_str(&json).unwrap();
    assert_eq!(sd, recovered);
}

#[test]
fn b035_snapshot_diff_serde_roundtrip() {
    let sd = SnapshotDiff {
        from_sequence: 1, to_sequence: 5,
        instance_id: InstanceId::from_bytes([7; 16]),
        state_diff: StateDiff { counter: DiffOperation::Added(100) },
    };
    let json = serde_json::to_string(&sd).unwrap();
    let recovered: SnapshotDiff = serde_json::from_str(&json).unwrap();
    assert_eq!(sd, recovered);
}

#[test]
fn b036_diff_result_serde_roundtrip() {
    let cases = vec![
        DiffResult::Identical,
        DiffResult::HasDiff(SnapshotDiff {
            from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
            state_diff: StateDiff { counter: DiffOperation::Modified(10, 20) },
        }),
    ];
    for dr in cases {
        let json = serde_json::to_string(&dr).unwrap();
        let recovered: DiffResult = serde_json::from_str(&json).unwrap();
        assert_eq!(dr, recovered, "roundtrip failed for {dr:?}");
    }
}

// ═══════════════════════════════════════════════════════════════
// B-037: DiffError Display format
// ═══════════════════════════════════════════════════════════════

#[test]
fn b037_diff_error_display() {
    assert_eq!(format!("{}", DiffError::CorruptSnapshot), "Snapshot bytes fail deserialization");
    assert_eq!(format!("{}", DiffError::VersionMismatch), "Schema version incompatibility");
    assert_eq!(format!("{}", DiffError::SequenceGap), "Snapshots not consecutive");
    assert_eq!(format!("{}", DiffError::SerializationFailed), "Cannot serialize diff");
    assert_eq!(format!("{}", DiffError::DeserializationFailed), "Cannot deserialize diff");
}

// ═══════════════════════════════════════════════════════════════
// B-038: ApplyError Display format
// ═══════════════════════════════════════════════════════════════

#[test]
fn b038_apply_error_display() {
    assert_eq!(format!("{}", ApplyError::BaseStateMismatch), "Base state doesn't match expected");
    assert_eq!(format!("{}", ApplyError::DiffTargetInvalid), "Diff cannot apply to base");
    assert_eq!(format!("{}", ApplyError::SequenceRegress), "Target sequence < base sequence");
}

// ═══════════════════════════════════════════════════════════════
// B-039: DiffResult Debug output distinguishes variants
// ═══════════════════════════════════════════════════════════════

#[test]
fn b039_diff_result_debug_distinguishes_variants() {
    assert!(format!("{:?}", DiffResult::Identical).contains("Identical"));
    let has_diff = DiffResult::HasDiff(SnapshotDiff {
        from_sequence: 0, to_sequence: 5, instance_id: test_id(1),
        state_diff: StateDiff { counter: DiffOperation::Added(42) },
    });
    assert!(format!("{has_diff:?}").contains("HasDiff"));
}

// ═══════════════════════════════════════════════════════════════
// Proptest Invariants
// ═══════════════════════════════════════════════════════════════

// PI-001: diff idempotency (INV-DIFF-1)
proptest::proptest! {
    #[test]
    fn pi001_diff_returns_identical_for_equal_sequences(
        seq: u64,
        counter_a: u64,
        counter_b: u64,
    ) {
        let result = diff(
            test_id(1),
            &(seq, InstanceState { counter: counter_a }),
            &(seq, InstanceState { counter: counter_b }),
        );
        proptest::prop_assert!(matches!(result, DiffResult::Identical));
    }
}

// PI-002: diff-apply roundtrip (INV-DIFF-2)
proptest::proptest! {
    #[test]
    fn pi002_diff_apply_roundtrip(
        from_seq: u64,
        to_seq_delta: u64,
        from_counter: u64,
        to_counter: u64,
    ) {
        let to_seq = if to_seq_delta == 0 {
            from_seq
        } else if to_seq_delta > u64::MAX - from_seq {
            from_seq
        } else {
            from_seq + to_seq_delta
        };
        let from_state = InstanceState { counter: from_counter };
        let to_state = InstanceState { counter: to_counter };
        let diff_result = diff(test_id(1), &(from_seq, from_state.clone()), &(to_seq, to_state.clone()));
        match diff_result {
            DiffResult::HasDiff(d) => {
                let applied = apply_diff(&(from_seq, from_state), &d);
                proptest::prop_assert!(applied.is_ok());
                proptest::prop_assert_eq!(applied.unwrap().counter, to_counter);
            }
            DiffResult::Identical => {
                proptest::prop_assert_eq!(from_seq, to_seq);
            }
        }
    }
}

// PI-003: invert-diff roundtrip (INV-DIFF-3)
proptest::proptest! {
    #[test]
    fn pi003_invert_diff_roundtrip(
        from_seq: u64,
        to_seq: u64,
        counter_val: u64,
    ) {
        let to_seq = if to_seq <= from_seq { from_seq + 1 } else { to_seq };
        let from_state = InstanceState { counter: counter_val };
        let to_state = InstanceState { counter: counter_val.wrapping_add(1) };
        let diff_result = diff(test_id(1), &(from_seq, from_state.clone()), &(to_seq, to_state));
        if let DiffResult::HasDiff(d) = diff_result {
            let inverted = invert_diff(&d);
            let applied = apply_diff(
                &(to_seq, InstanceState { counter: counter_val.wrapping_add(1) }),
                &inverted,
            );
            proptest::prop_assert!(applied.is_ok());
            proptest::prop_assert_eq!(applied.unwrap().counter, counter_val);
        }
    }
}

// PI-004: compose associativity (INV-DIFF-4)
proptest::proptest! {
    #[test]
    fn pi004_compose_associativity(
        seq_a: u64,
        counter_a: u64,
        counter_b: u64,
        counter_c: u64,
    ) {
        if seq_a > u64::MAX - 2 { return Ok(()); }
        let seq_b = seq_a + 1;
        let seq_c = seq_a + 2;
        let state_a = InstanceState { counter: counter_a };
        let state_b = InstanceState { counter: counter_b };
        let state_c = InstanceState { counter: counter_c };

        let diff_ab = diff(test_id(1), &(seq_a, state_a.clone()), &(seq_b, state_b.clone()));
        let diff_bc = diff(test_id(1), &(seq_b, state_b.clone()), &(seq_c, state_c.clone()));
        let diff_ac = diff(test_id(1), &(seq_a, state_a.clone()), &(seq_c, state_c.clone()));

        if let (DiffResult::HasDiff(d_ab), DiffResult::HasDiff(d_bc)) = (diff_ab, diff_bc) {
            let composed = d_ab.compose(&d_bc);
            match (composed, diff_ac) {
                (Ok(comp), DiffResult::HasDiff(direct)) => {
                    proptest::prop_assert_eq!(comp.state_diff, direct.state_diff);
                    proptest::prop_assert_eq!(comp.from_sequence, direct.from_sequence);
                    proptest::prop_assert_eq!(comp.to_sequence, direct.to_sequence);
                }
                (Err(_), _) => {}
                (_, DiffResult::Identical) => {}
            }
        }
    }
}

// PI-005: invert is involutory (self-inverse)
proptest::proptest! {
    #[test]
    fn pi005_invert_diff_is_involutory(
        from_seq: u64,
        to_seq: u64,
        counter_op: (u8, u64, u64),
    ) {
        let op = match counter_op {
            (0, _, _) => DiffOperation::Unchanged,
            (1, v, _) => DiffOperation::Added(v),
            (2, v, _) => DiffOperation::Removed(v),
            (_, a, b) => DiffOperation::Modified(a, b),
        };
        let d = SnapshotDiff {
            from_sequence: from_seq,
            to_sequence: to_seq,
            instance_id: test_id(1),
            state_diff: StateDiff { counter: op },
        };
        let double_inverted = invert_diff(&invert_diff(&d));
        proptest::prop_assert_eq!(d, double_inverted);
    }
}
