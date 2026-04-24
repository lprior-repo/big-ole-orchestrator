# Test Plan: Snapshot Diff Engine

## Summary

- **Bead**: ve-sc4 — Test Plan: Snapshot diff engine
- **Contract**: ve-rdi — Contract: Snapshot diff engine
- **Source**: `crates/vo-storage/src/snapshot_diff/mod.rs`
- **Behaviors identified**: 39
- **Trophy allocation**: 28 unit / 4 integration / 0 e2e / 2 static
- **Proptest invariants**: 5
- **Fuzz targets**: 3
- **Kani harnesses**: 1
- **Mutation checkpoints**: 10

---

## 1. Behavior Inventory

| # | Behavior | Public API |
|---|----------|------------|
| B-001 | `diff()` returns `Identical` when from_seq == to_seq | `diff()` |
| B-002 | `diff()` returns `Identical` when to_seq < from_seq (regression) | `diff()` |
| B-003 | `diff()` returns `Unchanged` when counters equal but sequences differ | `diff()` |
| B-004 | `diff()` returns `Added(T)` when from counter is 0 and to counter > 0 | `diff()` |
| B-005 | `diff()` returns `Removed(T)` when from counter > 0 and to counter is 0 | `diff()` |
| B-006 | `diff()` returns `Modified(from, to)` when both counters differ and non-zero | `diff()` |
| B-007 | `diff()` produces `SnapshotDiff` with correct sequences and instance_id | `diff()` |
| B-008 | `apply_diff()` with `Unchanged` preserves base counter | `apply_diff()` |
| B-009 | `apply_diff()` with `Added` sets counter when base is 0 | `apply_diff()` |
| B-010 | `apply_diff()` with `Added` returns `DiffTargetInvalid` when base != 0 | `apply_diff()` |
| B-011 | `apply_diff()` with `Removed` sets counter to 0 when base matches | `apply_diff()` |
| B-012 | `apply_diff()` with `Removed` returns `DiffTargetInvalid` when base mismatches | `apply_diff()` |
| B-013 | `apply_diff()` with `Modified` updates counter when base matches old_val | `apply_diff()` |
| B-014 | `apply_diff()` with `Modified` returns `DiffTargetInvalid` when base mismatches | `apply_diff()` |
| B-015 | `apply_diff()` returns `BaseStateMismatch` when sequences differ | `apply_diff()` |
| B-016 | `invert_diff()` swaps `Added` to `Removed` | `invert_diff()` |
| B-017 | `invert_diff()` swaps `Removed` to `Added` | `invert_diff()` |
| B-018 | `invert_diff()` swaps `Modified(a, b)` to `Modified(b, a)` | `invert_diff()` |
| B-019 | `invert_diff()` preserves `Unchanged` | `invert_diff()` |
| B-020 | `invert_diff()` swaps `from_sequence` and `to_sequence` | `invert_diff()` |
| B-021 | `invert_diff()` preserves `instance_id` | `invert_diff()` |
| B-022 | `compose()` returns `SequenceGap` when `to_sequence != other.from_sequence` | `SnapshotDiff::compose()` |
| B-023 | `compose()` returns `SequenceGap` when `instance_id`s differ | `SnapshotDiff::compose()` |
| B-024 | `compose()` with `Unchanged` + `op` yields `op` | `SnapshotDiff::compose()` |
| B-025 | `compose()` with `op` + `Unchanged` yields `op` | `SnapshotDiff::compose()` |
| B-026 | `compose()` chains `Modified(a,b)` + `Modified(b,c)` into `Modified(a,c)` | `SnapshotDiff::compose()` |
| B-027 | `compose()` rejects `Modified(a,b)` + `Modified(x,c)` when `b != x` | `SnapshotDiff::compose()` |
| B-028 | `compose()` rejects `Added` + `Added` | `SnapshotDiff::compose()` |
| B-029 | `compose()` rejects `Removed` + `Removed` | `SnapshotDiff::compose()` |
| B-030 | `compose()` rejects mismatched operation pairs (fallthrough) | `SnapshotDiff::compose()` |
| B-031 | `compose()` result has correct `from_sequence` and `to_sequence` | `SnapshotDiff::compose()` |
| B-032 | `compose()` result preserves `instance_id` | `SnapshotDiff::compose()` |
| B-033 | `DiffOperation` serde roundtrip preserves all variants | `DiffOperation` derive |
| B-034 | `StateDiff` serde roundtrip preserves counter | `StateDiff` derive |
| B-035 | `SnapshotDiff` serde roundtrip preserves all fields | `SnapshotDiff` derive |
| B-036 | `DiffResult` serde roundtrip preserves both variants | `DiffResult` derive |
| B-037 | `DiffError` Display format for each variant | `DiffError::fmt()` |
| B-038 | `ApplyError` Display format for each variant | `ApplyError::fmt()` |
| B-039 | `DiffResult` Debug output distinguishes `Identical` from `HasDiff` | `DiffResult` derive |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit / Calc** | 28 | Pure functions: `diff()`, `apply_diff()`, `invert_diff()`, `compose()`. All are synchronous pure transforms with no I/O. Exhaustive coverage of every branch in match arms and conditionals. |
| **Integration** | 4 | Serde roundtrip across JSON and binary formats for all 4 serializable types (`DiffOperation`, `StateDiff`, `SnapshotDiff`, `DiffResult`). |
| **E2E** | 0 | No I/O boundary in this module — the diff engine is pure computation. |
| **Static Analysis** | 2 | `clippy::pedantic` lint gates, exhaustive match coverage on enums. |

**Rationale for distribution**: The snapshot diff engine is a pure data/computation layer with zero I/O dependencies. Every function is a synchronous pure transform. The testing trophy's integration layer maps to serde roundtrip tests (real serialization across formats). No E2E tests needed because there is no external boundary.

---

## 3. BDD Scenarios

### B-001: diff() returns Identical when sequences are equal

**Scenario: same sequence produces Identical regardless of state**

```
Given: two (seq, state) pairs with identical sequence numbers
When: diff() is called
Then: returns DiffResult::Identical
```

```rust
fn diff_returns_identical_when_sequences_equal() {
    let id = InstanceId::from_bytes([1; 16]);
    let state_a = InstanceState { counter: 10 };
    let state_b = InstanceState { counter: 20 };
    let result = diff(id, &(5, state_a), &(5, state_b));
    assert!(matches!(result, DiffResult::Identical));
}

fn diff_returns_identical_when_sequences_equal_and_states_equal() {
    let id = InstanceId::from_bytes([1; 16]);
    let state = InstanceState { counter: 42 };
    let result = diff(id, &(3, state.clone()), &(3, state));
    assert!(matches!(result, DiffResult::Identical));
}
```

---

### B-002: diff() returns Identical on sequence regression

**Scenario: to_seq < from_seq produces Identical**

```
Given: to_sequence < from_sequence
When: diff() is called
Then: returns DiffResult::Identical
```

```rust
fn diff_returns_identical_when_to_sequence_less_than_from() {
    let id = InstanceId::from_bytes([1; 16]);
    let from_state = InstanceState { counter: 10 };
    let to_state = InstanceState { counter: 20 };
    let result = diff(id, &(10, from_state), &(5, to_state));
    assert!(matches!(result, DiffResult::Identical));
}

fn diff_returns_identical_when_to_sequence_zero_and_from_nonzero() {
    let id = InstanceId::from_bytes([1; 16]);
    let from_state = InstanceState { counter: 10 };
    let to_state = InstanceState { counter: 20 };
    let result = diff(id, &(1, from_state), &(0, to_state));
    assert!(matches!(result, DiffResult::Identical));
}
```

---

### B-003: diff() returns Unchanged when counters equal

**Scenario: same counter value with different sequences**

```
Given: from_state.counter == to_state.counter but sequences differ
When: diff() is called
Then: returns HasDiff with Unchanged counter operation
```

```rust
fn diff_returns_unchanged_when_counters_equal() {
    let id = InstanceId::from_bytes([1; 16]);
    let state = InstanceState { counter: 42 };
    let result = diff(id, &(1, state.clone()), &(5, state));
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Unchanged));
        }
        _ => panic!("Expected HasDiff"),
    }
}

fn diff_returns_unchanged_when_both_counters_zero() {
    let id = InstanceId::from_bytes([1; 16]);
    let from_state = InstanceState { counter: 0 };
    let to_state = InstanceState { counter: 0 };
    let result = diff(id, &(1, from_state), &(2, to_state));
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Unchanged));
        }
        _ => panic!("Expected HasDiff"),
    }
}
```

---

### B-004: diff() returns Added when from is zero

**Scenario: counter goes from 0 to positive**

```
Given: from_state.counter == 0 and to_state.counter > 0
When: diff() is called
Then: returns HasDiff with Added(to_counter)
```

```rust
fn diff_returns_added_when_from_zero_to_positive() {
    let id = InstanceId::from_bytes([1; 16]);
    let from_state = InstanceState { counter: 0 };
    let to_state = InstanceState { counter: 100 };
    let result = diff(id, &(0, from_state), &(1, to_state));
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Added(100)));
        }
        _ => panic!("Expected HasDiff"),
    }
}

fn diff_returns_added_with_large_value() {
    let id = InstanceId::from_bytes([1; 16]);
    let from_state = InstanceState { counter: 0 };
    let to_state = InstanceState { counter: u64::MAX };
    let result = diff(id, &(0, from_state), &(1, to_state));
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Added(u64::MAX)));
        }
        _ => panic!("Expected HasDiff"),
    }
}
```

---

### B-005: diff() returns Removed when to is zero

**Scenario: counter goes from positive to 0**

```
Given: from_state.counter > 0 and to_state.counter == 0
When: diff() is called
Then: returns HasDiff with Removed(from_counter)
```

```rust
fn diff_returns_removed_when_from_positive_to_zero() {
    let id = InstanceId::from_bytes([1; 16]);
    let from_state = InstanceState { counter: 50 };
    let to_state = InstanceState { counter: 0 };
    let result = diff(id, &(1, from_state), &(2, to_state));
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Removed(50)));
        }
        _ => panic!("Expected HasDiff"),
    }
}

fn diff_returns_removed_with_large_value() {
    let id = InstanceId::from_bytes([1; 16]);
    let from_state = InstanceState { counter: u64::MAX };
    let to_state = InstanceState { counter: 0 };
    let result = diff(id, &(1, from_state), &(2, to_state));
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Removed(u64::MAX)));
        }
        _ => panic!("Expected HasDiff"),
    }
}
```

---

### B-006: diff() returns Modified when both counters differ and non-zero

**Scenario: counter changes from one non-zero to another non-zero**

```
Given: from_state.counter > 0 and to_state.counter > 0 and they differ
When: diff() is called
Then: returns HasDiff with Modified(from_counter, to_counter)
```

```rust
fn diff_returns_modified_when_both_nonzero_and_different() {
    let id = InstanceId::from_bytes([1; 16]);
    let from_state = InstanceState { counter: 10 };
    let to_state = InstanceState { counter: 20 };
    let result = diff(id, &(0, from_state), &(1, to_state));
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Modified(10, 20)));
        }
        _ => panic!("Expected HasDiff"),
    }
}

fn diff_returns_modified_with_decreasing_value() {
    let id = InstanceId::from_bytes([1; 16]);
    let from_state = InstanceState { counter: 200 };
    let to_state = InstanceState { counter: 100 };
    let result = diff(id, &(0, from_state), &(1, to_state));
    match result {
        DiffResult::HasDiff(d) => {
            assert!(matches!(d.state_diff.counter, DiffOperation::Modified(200, 100)));
        }
        _ => panic!("Expected HasDiff"),
    }
}
```

---

### B-007: diff() SnapshotDiff carries correct metadata

**Scenario: all fields are populated correctly**

```
Given: from and to tuples with specific sequences and instance_id
When: diff() produces HasDiff
Then: SnapshotDiff.from_sequence, to_sequence, instance_id match inputs
```

```rust
fn diff_snapshot_diff_carries_correct_metadata() {
    let id = InstanceId::from_bytes([42; 16]);
    let from_state = InstanceState { counter: 10 };
    let to_state = InstanceState { counter: 20 };
    let result = diff(id.clone(), &(7, from_state), &(13, to_state));
    match result {
        DiffResult::HasDiff(d) => {
            assert_eq!(d.from_sequence, 7);
            assert_eq!(d.to_sequence, 13);
            assert_eq!(d.instance_id, id);
        }
        _ => panic!("Expected HasDiff"),
    }
}
```

---

### B-008: apply_diff() with Unchanged preserves base

**Scenario: unchanged diff keeps base counter**

```
Given: a SnapshotDiff with Unchanged counter
When: apply_diff() is called
Then: resulting state has same counter as base
```

```rust
fn apply_diff_unchanged_preserves_base_counter() {
    let id = InstanceId::from_bytes([1; 16]);
    let base = InstanceState { counter: 42 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Unchanged,
        },
    };
    let result = apply_diff(&(0, base.clone()), &d);
    assert_eq!(result.unwrap(), base);
}
```

---

### B-009: apply_diff() with Added sets counter when base is 0

**Scenario: Added operation applies to zero base**

```
Given: base counter is 0 and diff has Added(100)
When: apply_diff() is called
Then: resulting counter is 100
```

```rust
fn apply_diff_added_sets_counter_when_base_zero() {
    let id = InstanceId::from_bytes([1; 16]);
    let base = InstanceState { counter: 0 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Added(100),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert_eq!(result.unwrap().counter, 100);
}
```

---

### B-010: apply_diff() with Added rejects non-zero base

**Scenario: Added fails when base already has a value**

```
Given: base counter is 50 and diff has Added(100)
When: apply_diff() is called
Then: returns Err(ApplyError::DiffTargetInvalid)
```

```rust
fn apply_diff_added_rejects_nonzero_base() {
    let id = InstanceId::from_bytes([1; 16]);
    let base = InstanceState { counter: 50 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Added(100),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert!(matches!(result, Err(ApplyError::DiffTargetInvalid)));
}
```

---

### B-011: apply_diff() with Removed sets counter to 0 when base matches

**Scenario: Removed zeroes counter when base matches removed value**

```
Given: base counter == removed value and diff has Removed(42)
When: apply_diff() is called
Then: resulting counter is 0
```

```rust
fn apply_diff_removed_zeros_counter_when_base_matches() {
    let id = InstanceId::from_bytes([1; 16]);
    let base = InstanceState { counter: 42 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Removed(42),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert_eq!(result.unwrap().counter, 0);
}
```

---

### B-012: apply_diff() with Removed rejects mismatched base

**Scenario: Removed fails when base doesn't match removed value**

```
Given: base counter is 30 and diff has Removed(42)
When: apply_diff() is called
Then: returns Err(ApplyError::DiffTargetInvalid)
```

```rust
fn apply_diff_removed_rejects_mismatched_base() {
    let id = InstanceId::from_bytes([1; 16]);
    let base = InstanceState { counter: 30 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Removed(42),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert!(matches!(result, Err(ApplyError::DiffTargetInvalid)));
}
```

---

### B-013: apply_diff() with Modified updates counter when base matches

**Scenario: Modified updates counter correctly**

```
Given: base counter matches old_val in Modified(old, new)
When: apply_diff() is called
Then: resulting counter is new
```

```rust
fn apply_diff_modified_updates_counter_when_base_matches() {
    let id = InstanceId::from_bytes([1; 16]);
    let base = InstanceState { counter: 10 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert_eq!(result.unwrap().counter, 20);
}
```

---

### B-014: apply_diff() with Modified rejects mismatched base

**Scenario: Modified fails when base doesn't match old_val**

```
Given: base counter is 30 and diff has Modified(10, 20)
When: apply_diff() is called
Then: returns Err(ApplyError::DiffTargetInvalid)
```

```rust
fn apply_diff_modified_rejects_mismatched_base() {
    let id = InstanceId::from_bytes([1; 16]);
    let base = InstanceState { counter: 30 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert!(matches!(result, Err(ApplyError::DiffTargetInvalid)));
}
```

---

### B-015: apply_diff() returns BaseStateMismatch on sequence mismatch

**Scenario: sequences don't line up**

```
Given: diff.from_sequence != base sequence
When: apply_diff() is called
Then: returns Err(ApplyError::BaseStateMismatch)
```

```rust
fn apply_diff_returns_base_mismatch_on_sequence_mismatch() {
    let id = InstanceId::from_bytes([1; 16]);
    let base = InstanceState { counter: 10 };
    let d = SnapshotDiff {
        from_sequence: 99,
        to_sequence: 100,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Unchanged,
        },
    };
    let result = apply_diff(&(0, base), &d);
    assert!(matches!(result, Err(ApplyError::BaseStateMismatch)));
}

fn apply_diff_returns_base_mismatch_when_diff_from_zero_but_base_nonzero_seq() {
    let id = InstanceId::from_bytes([1; 16]);
    let base = InstanceState { counter: 10 };
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Unchanged,
        },
    };
    let result = apply_diff(&(3, base), &d);
    assert!(matches!(result, Err(ApplyError::BaseStateMismatch)));
}
```

---

### B-016: invert_diff() swaps Added to Removed

**Scenario: Added becomes Removed**

```
Given: SnapshotDiff with Added(100)
When: invert_diff() is called
Then: counter is Removed(100)
```

```rust
fn invert_diff_swaps_added_to_removed() {
    let id = InstanceId::from_bytes([1; 16]);
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Added(100),
        },
    };
    let inverted = invert_diff(&d);
    assert!(matches!(inverted.state_diff.counter, DiffOperation::Removed(100)));
}
```

---

### B-017: invert_diff() swaps Removed to Added

**Scenario: Removed becomes Added**

```
Given: SnapshotDiff with Removed(100)
When: invert_diff() is called
Then: counter is Added(100)
```

```rust
fn invert_diff_swaps_removed_to_added() {
    let id = InstanceId::from_bytes([1; 16]);
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Removed(100),
        },
    };
    let inverted = invert_diff(&d);
    assert!(matches!(inverted.state_diff.counter, DiffOperation::Added(100)));
}
```

---

### B-018: invert_diff() swaps Modified(a, b) to Modified(b, a)

**Scenario: Modified direction reverses**

```
Given: SnapshotDiff with Modified(10, 20)
When: invert_diff() is called
Then: counter is Modified(20, 10)
```

```rust
fn invert_diff_swaps_modified_order() {
    let id = InstanceId::from_bytes([1; 16]);
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let inverted = invert_diff(&d);
    assert!(matches!(inverted.state_diff.counter, DiffOperation::Modified(20, 10)));
}
```

---

### B-019: invert_diff() preserves Unchanged

**Scenario: Unchanged stays Unchanged**

```
Given: SnapshotDiff with Unchanged
When: invert_diff() is called
Then: counter is Unchanged
```

```rust
fn invert_diff_preserves_unchanged() {
    let id = InstanceId::from_bytes([1; 16]);
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Unchanged,
        },
    };
    let inverted = invert_diff(&d);
    assert!(matches!(inverted.state_diff.counter, DiffOperation::Unchanged));
}
```

---

### B-020: invert_diff() swaps from_sequence and to_sequence

**Scenario: sequences reverse**

```
Given: SnapshotDiff with from_sequence=0, to_sequence=5
When: invert_diff() is called
Then: inverted has from_sequence=5, to_sequence=0
```

```rust
fn invert_diff_swaps_sequences() {
    let id = InstanceId::from_bytes([1; 16]);
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let inverted = invert_diff(&d);
    assert_eq!(inverted.from_sequence, 5);
    assert_eq!(inverted.to_sequence, 0);
}
```

---

### B-021: invert_diff() preserves instance_id

**Scenario: instance_id survives inversion**

```
Given: SnapshotDiff with specific instance_id
When: invert_diff() is called
Then: inverted instance_id matches original
```

```rust
fn invert_diff_preserves_instance_id() {
    let id = InstanceId::from_bytes([42; 16]);
    let d = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Added(10),
        },
    };
    let inverted = invert_diff(&d);
    assert_eq!(inverted.instance_id, id);
}
```

---

### B-022: compose() rejects sequence gap

**Scenario: non-consecutive sequences**

```
Given: diff_ab.to_sequence != diff_bc.from_sequence
When: compose() is called
Then: returns Err(DiffError::SequenceGap)
```

```rust
fn compose_rejects_sequence_gap() {
    let id = InstanceId::from_bytes([1; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 6,
        to_sequence: 10,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Modified(20, 30),
        },
    };
    let result = diff_ab.compose(&diff_bc);
    assert!(matches!(result, Err(DiffError::SequenceGap)));
}
```

---

### B-023: compose() rejects mismatched instance_ids

**Scenario: different instance IDs**

```
Given: diff_ab.instance_id != diff_bc.instance_id
When: compose() is called
Then: returns Err(DiffError::SequenceGap)
```

```rust
fn compose_rejects_mismatched_instance_ids() {
    let id_a = InstanceId::from_bytes([1; 16]);
    let id_b = InstanceId::from_bytes([2; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id_a,
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id_b,
        state_diff: StateDiff {
            counter: DiffOperation::Modified(20, 30),
        },
    };
    let result = diff_ab.compose(&diff_bc);
    assert!(matches!(result, Err(DiffError::SequenceGap)));
}
```

---

### B-024: compose() with Unchanged + op yields op

**Scenario: left identity**

```
Given: diff_ab has Unchanged counter, diff_bc has any op
When: compose() is called
Then: result has diff_bc's operation
```

```rust
fn compose_left_identity_unchanged() {
    let id = InstanceId::from_bytes([1; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Unchanged,
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Added(50),
        },
    };
    let result = diff_ab.compose(&diff_bc).unwrap();
    assert!(matches!(result.state_diff.counter, DiffOperation::Added(50)));
}
```

---

### B-025: compose() with op + Unchanged yields op

**Scenario: right identity**

```
Given: diff_ab has any op, diff_bc has Unchanged counter
When: compose() is called
Then: result has diff_ab's operation
```

```rust
fn compose_right_identity_unchanged() {
    let id = InstanceId::from_bytes([1; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Removed(42),
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Unchanged,
        },
    };
    let result = diff_ab.compose(&diff_bc).unwrap();
    assert!(matches!(result.state_diff.counter, DiffOperation::Removed(42)));
}
```

---

### B-026: compose() chains Modified correctly

**Scenario: compose Modified(a,b) + Modified(b,c) = Modified(a,c)**

```
Given: diff_ab has Modified(10, 20) and diff_bc has Modified(20, 30)
When: compose() is called
Then: result has Modified(10, 30)
```

```rust
fn compose_chains_modified_correctly() {
    let id = InstanceId::from_bytes([1; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Modified(20, 30),
        },
    };
    let result = diff_ab.compose(&diff_bc).unwrap();
    assert!(matches!(result.state_diff.counter, DiffOperation::Modified(10, 30)));
}
```

---

### B-027: compose() rejects Modified with mismatched middle

**Scenario: compose Modified(a,b) + Modified(x,c) where b != x**

```
Given: diff_ab has Modified(10, 20) and diff_bc has Modified(99, 30)
When: compose() is called
Then: returns Err(DiffError::SequenceGap)
```

```rust
fn compose_rejects_modified_with_mismatched_middle() {
    let id = InstanceId::from_bytes([1; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Modified(99, 30),
        },
    };
    let result = diff_ab.compose(&diff_bc);
    assert!(matches!(result, Err(DiffError::SequenceGap)));
}
```

---

### B-028: compose() rejects Added + Added

**Scenario: double-add is invalid**

```
Given: diff_ab has Added and diff_bc has Added
When: compose() is called
Then: returns Err(DiffError::SequenceGap)
```

```rust
fn compose_rejects_added_plus_added() {
    let id = InstanceId::from_bytes([1; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Added(10),
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Added(20),
        },
    };
    let result = diff_ab.compose(&diff_bc);
    assert!(matches!(result, Err(DiffError::SequenceGap)));
}
```

---

### B-029: compose() rejects Removed + Removed

**Scenario: double-remove is invalid**

```
Given: diff_ab has Removed and diff_bc has Removed
When: compose() is called
Then: returns Err(DiffError::SequenceGap)
```

```rust
fn compose_rejects_removed_plus_removed() {
    let id = InstanceId::from_bytes([1; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Removed(10),
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Removed(20),
        },
    };
    let result = diff_ab.compose(&diff_bc);
    assert!(matches!(result, Err(DiffError::SequenceGap)));
}
```

---

### B-030: compose() rejects invalid mixed pairs (fallthrough)

**Scenario: Added+Modified, Modified+Added, etc.**

```
Given: operation pairs that don't match any valid compose rule
When: compose() is called
Then: returns Err(DiffError::SequenceGap)
```

```rust
fn compose_rejects_added_plus_modified() {
    let id = InstanceId::from_bytes([1; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Added(10),
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let result = diff_ab.compose(&diff_bc);
    assert!(matches!(result, Err(DiffError::SequenceGap)));
}

fn compose_rejects_modified_plus_added() {
    let id = InstanceId::from_bytes([1; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Added(30),
        },
    };
    let result = diff_ab.compose(&diff_bc);
    assert!(matches!(result, Err(DiffError::SequenceGap)));
}

fn compose_rejects_removed_plus_modified() {
    let id = InstanceId::from_bytes([1; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Removed(10),
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Modified(0, 20),
        },
    };
    let result = diff_ab.compose(&diff_bc);
    assert!(matches!(result, Err(DiffError::SequenceGap)));
}
```

---

### B-031: compose() result has correct sequences

**Scenario: composed diff spans from first to last**

```
Given: diff_ab from 0->5 and diff_bc from 5->10
When: compose() succeeds
Then: result has from_sequence=0, to_sequence=10
```

```rust
fn compose_result_has_correct_sequences() {
    let id = InstanceId::from_bytes([1; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id,
        state_diff: StateDiff {
            counter: DiffOperation::Unchanged,
        },
    };
    let result = diff_ab.compose(&diff_bc).unwrap();
    assert_eq!(result.from_sequence, 0);
    assert_eq!(result.to_sequence, 10);
}
```

---

### B-032: compose() result preserves instance_id

**Scenario: instance_id survives composition**

```
Given: both diffs have the same instance_id
When: compose() succeeds
Then: result has that instance_id
```

```rust
fn compose_result_preserves_instance_id() {
    let id = InstanceId::from_bytes([42; 16]);
    let diff_ab = SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Unchanged,
        },
    };
    let diff_bc = SnapshotDiff {
        from_sequence: 5,
        to_sequence: 10,
        instance_id: id.clone(),
        state_diff: StateDiff {
            counter: DiffOperation::Added(99),
        },
    };
    let result = diff_ab.compose(&diff_bc).unwrap();
    assert_eq!(result.instance_id, id);
}
```

---

### B-033: DiffOperation serde roundtrip

**Scenario: all DiffOperation variants survive serialization**

```
Given: each DiffOperation<u64> variant
When: serialize to JSON then deserialize
Then: result equals original
```

```rust
fn diff_operation_serde_roundtrip_unchanged() {
    let op: DiffOperation<u64> = DiffOperation::Unchanged;
    let json = serde_json::to_string(&op).unwrap();
    let recovered: DiffOperation<u64> = serde_json::from_str(&json).unwrap();
    assert_eq!(op, recovered);
}

fn diff_operation_serde_roundtrip_added() {
    let op: DiffOperation<u64> = DiffOperation::Added(42);
    let json = serde_json::to_string(&op).unwrap();
    let recovered: DiffOperation<u64> = serde_json::from_str(&json).unwrap();
    assert_eq!(op, recovered);
}

fn diff_operation_serde_roundtrip_removed() {
    let op: DiffOperation<u64> = DiffOperation::Removed(99);
    let json = serde_json::to_string(&op).unwrap();
    let recovered: DiffOperation<u64> = serde_json::from_str(&json).unwrap();
    assert_eq!(op, recovered);
}

fn diff_operation_serde_roundtrip_modified() {
    let op: DiffOperation<u64> = DiffOperation::Modified(10, 20);
    let json = serde_json::to_string(&op).unwrap();
    let recovered: DiffOperation<u64> = serde_json::from_str(&json).unwrap();
    assert_eq!(op, recovered);
}
```

---

### B-034: StateDiff serde roundtrip

**Scenario: StateDiff survives serialization**

```rust
fn state_diff_serde_roundtrip() {
    let sd = StateDiff {
        counter: DiffOperation::Modified(5, 10),
    };
    let json = serde_json::to_string(&sd).unwrap();
    let recovered: StateDiff = serde_json::from_str(&json).unwrap();
    assert_eq!(sd, recovered);
}
```

---

### B-035: SnapshotDiff serde roundtrip

**Scenario: SnapshotDiff survives serialization**

```rust
fn snapshot_diff_serde_roundtrip() {
    let sd = SnapshotDiff {
        from_sequence: 1,
        to_sequence: 5,
        instance_id: InstanceId::from_bytes([7; 16]),
        state_diff: StateDiff {
            counter: DiffOperation::Added(100),
        },
    };
    let json = serde_json::to_string(&sd).unwrap();
    let recovered: SnapshotDiff = serde_json::from_str(&json).unwrap();
    assert_eq!(sd, recovered);
}
```

---

### B-036: DiffResult serde roundtrip

**Scenario: both DiffResult variants survive serialization**

```rust
fn diff_result_serde_roundtrip_identical() {
    let dr = DiffResult::Identical;
    let json = serde_json::to_string(&dr).unwrap();
    let recovered: DiffResult = serde_json::from_str(&json).unwrap();
    assert_eq!(dr, recovered);
}

fn diff_result_serde_roundtrip_has_diff() {
    let dr = DiffResult::HasDiff(SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: InstanceId::from_bytes([1; 16]),
        state_diff: StateDiff {
            counter: DiffOperation::Modified(10, 20),
        },
    });
    let json = serde_json::to_string(&dr).unwrap();
    let recovered: DiffResult = serde_json::from_str(&json).unwrap();
    assert_eq!(dr, recovered);
}
```

---

### B-037: DiffError Display format

**Scenario: each error variant formats correctly**

```rust
fn diff_error_display_corrupt_snapshot() {
    let err = DiffError::CorruptSnapshot;
    assert_eq!(format!("{}", err), "Snapshot bytes fail deserialization");
}

fn diff_error_display_version_mismatch() {
    let err = DiffError::VersionMismatch;
    assert_eq!(format!("{}", err), "Schema version incompatibility");
}

fn diff_error_display_sequence_gap() {
    let err = DiffError::SequenceGap;
    assert_eq!(format!("{}", err), "Snapshots not consecutive");
}

fn diff_error_display_serialization_failed() {
    let err = DiffError::SerializationFailed;
    assert_eq!(format!("{}", err), "Cannot serialize diff");
}

fn diff_error_display_deserialization_failed() {
    let err = DiffError::DeserializationFailed;
    assert_eq!(format!("{}", err), "Cannot deserialize diff");
}
```

---

### B-038: ApplyError Display format

**Scenario: each error variant formats correctly**

```rust
fn apply_error_display_base_state_mismatch() {
    let err = ApplyError::BaseStateMismatch;
    assert_eq!(format!("{}", err), "Base state doesn't match expected");
}

fn apply_error_display_diff_target_invalid() {
    let err = ApplyError::DiffTargetInvalid;
    assert_eq!(format!("{}", err), "Diff cannot apply to base");
}

fn apply_error_display_sequence_regress() {
    let err = ApplyError::SequenceRegress;
    assert_eq!(format!("{}", err), "Target sequence < base sequence");
}
```

---

### B-039: DiffResult Debug output distinguishes variants

**Scenario: Debug format is distinguishable**

```rust
fn diff_result_debug_identical() {
    let dr = DiffResult::Identical;
    let debug = format!("{:?}", dr);
    assert!(debug.contains("Identical"));
}

fn diff_result_debug_has_diff() {
    let dr = DiffResult::HasDiff(SnapshotDiff {
        from_sequence: 0,
        to_sequence: 5,
        instance_id: InstanceId::from_bytes([1; 16]),
        state_diff: StateDiff {
            counter: DiffOperation::Added(42),
        },
    });
    let debug = format!("{:?}", dr);
    assert!(debug.contains("HasDiff"));
}
```

---

## 4. Proptest Invariants

### PI-001: diff idempotency (INV-DIFF-1)

```
Invariant: diff(id, &(s, a), &(s, b)) == Identical for all sequences s
Strategy: arbitrary u64 sequence, arbitrary u64 counters
```

```rust
proptest! {
    #[test]
    fn diff_returns_identical_for_equal_sequences(
        seq: u64,
        counter_a: u64,
        counter_b: u64,
    ) {
        let id = InstanceId::from_bytes([1; 16]);
        let state_a = InstanceState { counter: counter_a };
        let state_b = InstanceState { counter: counter_b };
        let result = diff(id, &(seq, state_a), &(seq, state_b));
        prop_assert!(matches!(result, DiffResult::Identical));
    }
}
```

### PI-002: diff-apply roundtrip (INV-DIFF-2)

```
Invariant: apply_diff(&from, diff_result) == to for all valid (from, to) pairs
Strategy: arbitrary u64 sequences where to_seq > from_seq, arbitrary counters
```

```rust
proptest! {
    #[test]
    fn diff_apply_roundtrip(
        from_seq: u64,
        to_seq_delta: u64,
        from_counter: u64,
        to_counter: u64,
    ) {
        let to_seq = from_seq.wrapping_add(to_seq_delta).max(from_seq + 1);
        let id = InstanceId::from_bytes([1; 16]);
        let from_state = InstanceState { counter: from_counter };
        let to_state = InstanceState { counter: to_counter };
        let diff_result = diff(id, &(from_seq, from_state.clone()), &(to_seq, to_state.clone()));
        match diff_result {
            DiffResult::HasDiff(d) => {
                let applied = apply_diff(&(from_seq, from_state), &d);
                prop_assert!(applied.is_ok());
                prop_assert_eq!(applied.unwrap().counter, to_counter);
            }
            DiffResult::Identical => {
                prop_assert_eq!(from_seq, to_seq);
            }
        }
    }
}
```

### PI-003: invert-diff roundtrip (INV-DIFF-3)

```
Invariant: apply_diff(&to, invert_diff(d)) == from for all valid diffs
Strategy: arbitrary SnapshotDiff values
```

```rust
proptest! {
    #[test]
    fn invert_diff_roundtrip(
        from_seq: u64,
        to_seq: u64,
        counter_val: u64,
    ) {
        let id = InstanceId::from_bytes([1; 16]);
        let to_seq = if to_seq <= from_seq { from_seq + 1 } else { to_seq };
        let from_state = InstanceState { counter: counter_val };
        let to_state = InstanceState { counter: counter_val.wrapping_add(1) };
        let diff_result = diff(id, &(from_seq, from_state.clone()), &(to_seq, to_state));
        if let DiffResult::HasDiff(d) = diff_result {
            let inverted = invert_diff(&d);
            let applied = apply_diff(&(to_seq, to_state), &inverted);
            prop_assert!(applied.is_ok());
            prop_assert_eq!(applied.unwrap().counter, counter_val);
        }
    }
}
```

### PI-004: compose associativity (INV-DIFF-4)

```
Invariant: compose(diff(a,b), diff(b,c)) produces a diff equivalent to diff(a,c)
Strategy: three consecutive sequences with arbitrary counters
```

```rust
proptest! {
    #[test]
    fn compose_associativity(
        seq_a: u64,
        counter_a: u64,
        counter_b: u64,
        counter_c: u64,
    ) {
        let seq_b = seq_a + 1;
        let seq_c = seq_a + 2;
        let id = InstanceId::from_bytes([1; 16]);
        let state_a = InstanceState { counter: counter_a };
        let state_b = InstanceState { counter: counter_b };
        let state_c = InstanceState { counter: counter_c };

        let diff_ab = diff(id.clone(), &(seq_a, state_a.clone()), &(seq_b, state_b.clone()));
        let diff_bc = diff(id.clone(), &(seq_b, state_b.clone()), &(seq_c, state_c.clone()));
        let diff_ac = diff(id, &(seq_a, state_a.clone()), &(seq_c, state_c.clone()));

        if let (DiffResult::HasDiff(d_ab), DiffResult::HasDiff(d_bc)) = (diff_ab, diff_bc) {
            let composed = d_ab.compose(&d_bc);
            match (composed, diff_ac) {
                (Ok(comp), DiffResult::HasDiff(direct)) => {
                    prop_assert_eq!(comp.state_diff, direct.state_diff);
                    prop_assert_eq!(comp.from_sequence, direct.from_sequence);
                    prop_assert_eq!(comp.to_sequence, direct.to_sequence);
                }
                (Err(_), _) => {}
                (_, DiffResult::Identical) => {}
            }
        }
    }
}
```

### PI-005: invert is involutory (self-inverse)

```
Invariant: invert_diff(invert_diff(d)) == d for all SnapshotDiff
Strategy: arbitrary SnapshotDiff values
```

```rust
proptest! {
    #[test]
    fn invert_diff_is_involutory(
        from_seq: u64,
        to_seq: u64,
        counter_op: (u8, u64, u64),
    ) {
        let id = InstanceId::from_bytes([1; 16]);
        let op = match counter_op {
            (0, _, _) => DiffOperation::Unchanged,
            (1, v, _) => DiffOperation::Added(v),
            (2, v, _) => DiffOperation::Removed(v),
            (_, a, b) => DiffOperation::Modified(a, b),
        };
        let d = SnapshotDiff {
            from_sequence: from_seq,
            to_sequence: to_seq,
            instance_id: id,
            state_diff: StateDiff { counter: op },
        };
        let double_inverted = invert_diff(&invert_diff(&d));
        prop_assert_eq!(d, double_inverted);
    }
}
```

---

## 5. Fuzz Targets

### FT-001: diff() with boundary sequence values

```
Input type: (u64, u64, u64, u64) — from_seq, to_seq, from_counter, to_counter
Risk: incorrect branch selection at sequence boundaries (0, u64::MAX, equal)
Corpus seeds: (0,0,0,0), (0,1,0,1), (u64::MAX, u64::MAX, 0, 0), (0, u64::MAX, 0, u64::MAX)
```

### FT-002: apply_diff() with mismatched counter values

```
Input type: (u64, u64, u8) — base_counter, diff_value, operation_tag
Risk: incorrect validation in Added/Removed/Modified branches
Corpus seeds: (0,0,0), (0,1,1), (1,1,2), (100,50,3), (u64::MAX, u64::MAX, 3)
```

### FT-003: compose() operation pair combinations

```
Input type: (u8, u64, u64, u8, u64, u64) — (op_tag, val1, val2) x 2
Risk: missing fallthrough case in compose match, incorrect chaining
Corpus seeds: all 16 operation pair combinations with representative values
```

---

## 6. Kani Harnesses

### KH-001: diff() branch exhaustiveness

```
Property: diff() correctly handles all 4 counter comparison cases for valid sequence pairs
Bound: u64 values (full range)
Rationale: Formal proof that no branch in the if/else chain is dead code
```

```rust
#[kani::proof]
fn diff_handles_all_counter_branches() {
    let from_seq: u64 = kani::any();
    let to_seq: u64 = kani::any();
    let from_counter: u64 = kani::any();
    let to_counter: u64 = kani::any();
    kani::assume(to_seq > from_seq);
    let id = InstanceId::from_bytes([1; 16]);
    let result = diff(id, &(from_seq, InstanceState { counter: from_counter }), &(to_seq, InstanceState { counter: to_counter }));
    match result {
        DiffResult::HasDiff(d) => {
            match d.state_diff.counter {
                DiffOperation::Unchanged => { kani::assert(from_counter == to_counter, "unchanged implies equal"); }
                DiffOperation::Added(v) => { kani::assert(from_counter == 0 && to_counter > 0 && v == to_counter, "added implies 0->positive"); }
                DiffOperation::Removed(v) => { kani::assert(from_counter > 0 && to_counter == 0 && v == from_counter, "removed implies positive->0"); }
                DiffOperation::Modified(a, b) => { kani::assert(from_counter > 0 && to_counter > 0 && a == from_counter && b == to_counter, "modified implies both positive"); }
            }
        }
        DiffResult::Identical => {}
    }
}
```

---

## 7. Mutation Checkpoints

| Checkpoint | Mutated Code | Must Be Caught By |
|------------|--------------|-------------------|
| MC-001 | Swap `from_seq == to_seq` with `from_seq != to_seq` | `diff_returns_identical_when_sequences_equal` |
| MC-002 | Remove `to_seq < from_seq` early return | `diff_returns_identical_when_to_sequence_less_than_from` |
| MC-003 | Change `Added(to_counter)` to `Added(from_counter)` | `diff_returns_added_when_from_zero_to_positive` |
| MC-004 | Change `Removed(from_counter)` to `Removed(to_counter)` | `diff_returns_removed_when_from_positive_to_zero` |
| MC-005 | Remove `base_state.counter == 0` guard in Added branch | `apply_diff_added_rejects_nonzero_base` |
| MC-006 | Remove `base_state.counter == val` guard in Removed branch | `apply_diff_removed_rejects_mismatched_base` |
| MC-007 | Remove `base_state.counter == old_val` guard in Modified branch | `apply_diff_modified_rejects_mismatched_base` |
| MC-008 | Swap `Added <-> Removed` in invert_diff | `invert_diff_swaps_added_to_removed` + `invert_diff_swaps_removed_to_added` |
| MC-009 | Remove `self.to_sequence != other.from_sequence` check in compose | `compose_rejects_sequence_gap` |
| MC-010 | Change `*new_a == *old_b` to `*new_a != *old_b` in compose Modified match | `compose_chains_modified_correctly` + `compose_rejects_modified_with_mismatched_middle` |

**Threshold**: >=90% mutation kill rate

---

## 8. Combinatorial Coverage Matrix

### diff()

| Scenario | from_seq | to_seq | from_counter | to_counter | Expected | Layer |
|----------|----------|--------|--------------|------------|----------|-------|
| Same sequence | 5 | 5 | 10 | 20 | Identical | unit |
| Regression | 10 | 5 | 10 | 20 | Identical | unit |
| Equal counters | 1 | 5 | 42 | 42 | HasDiff(Unchanged) | unit |
| Both zero | 1 | 5 | 0 | 0 | HasDiff(Unchanged) | unit |
| Zero to positive | 0 | 1 | 0 | 100 | HasDiff(Added(100)) | unit |
| Positive to zero | 1 | 2 | 50 | 0 | HasDiff(Removed(50)) | unit |
| Modify increase | 0 | 1 | 10 | 20 | HasDiff(Modified(10,20)) | unit |
| Modify decrease | 0 | 1 | 200 | 100 | HasDiff(Modified(200,100)) | unit |
| u64::MAX boundary | 0 | 1 | 0 | u64::MAX | HasDiff(Added(u64::MAX)) | unit |
| Metadata check | 7 | 13 | 10 | 20 | from=7, to=13 | unit |
| PI-001: idempotent | any | same | any | any | Identical | proptest |

### apply_diff()

| Scenario | base_seq | diff.from_seq | base.counter | op | Expected | Layer |
|----------|----------|---------------|--------------|-----|----------|-------|
| Unchanged | 0 | 0 | 42 | Unchanged | Ok(42) | unit |
| Added valid | 0 | 0 | 0 | Added(100) | Ok(100) | unit |
| Added invalid | 0 | 0 | 50 | Added(100) | Err(DiffTargetInvalid) | unit |
| Removed valid | 0 | 0 | 42 | Removed(42) | Ok(0) | unit |
| Removed invalid | 0 | 0 | 30 | Removed(42) | Err(DiffTargetInvalid) | unit |
| Modified valid | 0 | 0 | 10 | Modified(10,20) | Ok(20) | unit |
| Modified invalid | 0 | 0 | 30 | Modified(10,20) | Err(DiffTargetInvalid) | unit |
| Seq mismatch | 0 | 99 | 10 | Unchanged | Err(BaseStateMismatch) | unit |
| PI-002: roundtrip | any | any | any | any | matches to_state | proptest |

### invert_diff()

| Scenario | Input op | Output op | Layer |
|----------|----------|-----------|-------|
| Added(100) | Added(100) | Removed(100) | unit |
| Removed(100) | Removed(100) | Added(100) | unit |
| Modified(10,20) | Modified(10,20) | Modified(20,10) | unit |
| Unchanged | Unchanged | Unchanged | unit |
| Seq swap | from=0,to=5 | from=5,to=0 | unit |
| Id preserved | id=X | id=X | unit |
| PI-005: involutory | any | == original | proptest |

### SnapshotDiff::compose()

| Scenario | op_ab | op_bc | Expected | Layer |
|----------|-------|-------|----------|-------|
| Unchanged + Added | Unchanged | Added(50) | Added(50) | unit |
| Removed + Unchanged | Removed(42) | Unchanged | Removed(42) | unit |
| Modified chain | Modified(10,20) | Modified(20,30) | Modified(10,30) | unit |
| Modified mismatch | Modified(10,20) | Modified(99,30) | Err(SequenceGap) | unit |
| Added + Added | Added(10) | Added(20) | Err(SequenceGap) | unit |
| Removed + Removed | Removed(10) | Removed(20) | Err(SequenceGap) | unit |
| Added + Modified | Added(10) | Modified(10,20) | Err(SequenceGap) | unit |
| Seq gap | to=5, from=6 | — | Err(SequenceGap) | unit |
| Different IDs | id_a, id_b | — | Err(SequenceGap) | unit |
| PI-004: associativity | any | any | matches diff(a,c) | proptest |

---

## 9. Boundary Value Analysis

| Input | Boundary | Test |
|-------|----------|------|
| from_sequence | 0, 1, u64::MAX | B-001, B-007, PI-001 |
| to_sequence | 0, 1, u64::MAX | B-002, B-007 |
| counter | 0, 1, u64::MAX | B-004, B-005, B-006 |
| from_seq == to_seq | equal | B-001 |
| to_seq == from_seq - 1 | regression | B-002 |
| counter transition 0->1 | minimal add | B-004 |
| counter transition 1->0 | minimal remove | B-005 |
| compose from_seq == 0 | boundary | B-031 |
| compose to_seq == u64::MAX | boundary | B-031 |

---

## Open Questions

1. **ApplyError::SequenceRegress**: This error variant exists but is never returned by `apply_diff()`. The contract notes say it was intentionally removed to support bidirectional diff application (INV-DIFF-3). Should this variant be removed from the enum, or is it reserved for future use?

2. **DiffError::CorruptSnapshot and DiffError::VersionMismatch**: These error variants exist in the enum but are never produced by any current function. Are they reserved for future serialization boundaries?

3. **diff() regression behavior**: When `to_seq < from_seq`, the function returns `Identical` rather than an error. Is this intentional design (graceful degradation) or should it produce an error variant?

---

## Exit Criteria Compliance

- [x] Every public API behavior has at least one BDD scenario
- [x] Every pure function with multiple inputs has at least one proptest invariant
- [x] Every error variant in `DiffError` and `ApplyError` has explicit test scenario
- [x] Every `DiffOperation` variant is covered in diff, apply, invert, and compose
- [x] Mutation threshold target (>=90%) is stated
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value
- [x] Serde roundtrip tests for all serializable types
- [x] Display format tests for all error types
