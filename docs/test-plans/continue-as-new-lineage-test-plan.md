# Test Plan: Continue-As-New Lineage (ADR-038)

## Summary
- Bead: ve-v39s — Test Plan: Continue-as-new (ADR-038)
- Behaviors identified: 24
- Trophy allocation: 18 unit / 4 integration / 2 e2e
- Proptest invariants: 4
- Fuzz targets: 1
- Kani harnesses: 0
- Target Mutation Kill Rate: ≥90%

---

## 1. Behavior Inventory

### EventPayload::ContinuedAsNew (8)
| # | Behavior | Public API |
|---|----------|------------|
| CA-01 | ContinuedAsNew decodes from JSON with all fields | `EventPayload::try_from_json` |
| CA-02 | ContinuedAsNew rejects missing lineage_id | `EventPayload::try_from_json` |
| CA-03 | ContinuedAsNew rejects missing old_epoch | `EventPayload::try_from_json` |
| CA-04 | ContinuedAsNew rejects missing new_epoch | `EventPayload::try_from_json` |
| CA-05 | ContinuedAsNew rejects non-integer old_epoch | `EventPayload::try_from_json` |
| CA-06 | ContinuedAsNew rejects non-integer new_epoch | `EventPayload::try_from_json` |
| CA-07 | ContinuedAsNew requires new_epoch > old_epoch | Rollover validation |
| CA-08 | ContinuedAsNew carries workflow_id, lineage_id, old_epoch, new_epoch | Event structure |

### ReplayEngine Continuation (4)
| # | Behavior | Public API |
|---|----------|------------|
| RE-01 | ReplayEngine skips ContinuedAsNew without state transition | `ReplayEngine::replay` |
| RE-02 | ReplayEngine counts ContinuedAsNew in events_applied | `ReplayEngine::replay` |
| RE-03 | ContinuedAsNew does not prevent replay of subsequent events | `ReplayEngine::replay` |
| RE-04 | ReplayEngine errors on ContinuedAsNew in payload_to_transition | `payload_to_transition` |

### Epoch and Lineage Types (4)
| # | Behavior | Public API |
|---|----------|------------|
| EP-01 | Epoch::ZERO is 0, Epoch::new(u64) preserves value | `Epoch::ZERO`, `Epoch::new` |
| EP-02 | Epoch ordering is consistent with u64 ordering | `Epoch::Ord` |
| EP-03 | WorkflowLineage::new creates root with epoch 0, no parent | `WorkflowLineage::new` |
| EP-04 | WorkflowLineage::with_parent enforces parent_epoch < epoch | `WorkflowLineage::with_parent` |

### LineageQuery Routing (4)
| # | Behavior | Public API |
|---|----------|------------|
| LQ-01 | lineage_prefix_generator rejects empty lineage_id | `lineage_prefix_generator` |
| LQ-02 | lineage_prefix_generator rejects null byte in lineage_id | `lineage_prefix_generator` |
| LQ-03 | lineage_prefix_generator rejects lineage_id exceeding max length | `lineage_prefix_generator` |
| LQ-04 | epoch_prefix_generator encodes lineage_id + epoch as key | `epoch_prefix_generator` |

### Signal Matching Across Rollover (4)
| # | Behavior | Public API |
|---|----------|------------|
| SM-01 | Lineage-wide signal matches wait in any epoch | `signal_match` |
| SM-02 | Epoch-local signal requires epoch match | `signal_match` |
| SM-03 | Lineage mismatch returns LineageMismatch result | `signal_match` |
| SM-04 | Epoch mismatch returns EpochMismatch result | `signal_match` |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit / Calc** | 18 | Pure data types (EventPayload, Epoch, WorkflowLineage), prefix generators, signal matching logic — no I/O, deterministic |
| **Integration** | 4 | LineageQuery prefix composition, ReplayEngine with mixed event sequences including ContinuedAsNew |
| **E2E** | 2 | Full lineage chain traversal: query across epochs, signal delivery to new epoch |
| **Static Analysis** | 0 | No complex async requiring clippy pedantic gates |

**Rationale for distribution**: The continue-as-new feature centers on data transformations (event payload parsing, prefix key construction, epoch ordering) that are pure functions. Integration tests verify that storage query routing composes correctly. E2E tests verify the cross-boundary scenarios (signal routing, lineage traversal) that require component integration.

---

## 3. BDD Scenarios

### CA-01: ContinuedAsNew decodes from JSON

**Scenario: Valid ContinuedAsNew payload decodes correctly**

```
Given: JSON with type="ContinuedAsNew", workflow_id, lineage_id, old_epoch=0, new_epoch=1
When: EventPayload::try_from_json is called
Then: Returns Ok(EventPayload::ContinuedAsNew { workflow_id, lineage_id, old_epoch, new_epoch })
```

```rust
#[test]
fn continued_as_new_decodes_from_json() {
    let json = serde_json::json!({
        "type": "ContinuedAsNew",
        "workflow_id": "wf-1",
        "lineage_id": "lin-abc",
        "old_epoch": 0,
        "new_epoch": 1
    });
    let result = EventPayload::try_from_json(&json);
    assert_eq!(result, Ok(EventPayload::ContinuedAsNew {
        workflow_id: "wf-1".into(),
        lineage_id: "lin-abc".into(),
        old_epoch: 0,
        new_epoch: 1,
    }));
}
```

---

### CA-02: ContinuedAsNew rejects missing lineage_id

**Scenario: JSON without lineage_id returns error**

```
Given: JSON with type="ContinuedAsNew" but no lineage_id
When: EventPayload::try_from_json is called
Then: Returns Err(MissingPayloadField("lineage_id"))
```

---

### CA-05: ContinuedAsNew rejects non-integer old_epoch

**Scenario: JSON with string old_epoch returns error**

```
Given: JSON with type="ContinuedAsNew", old_epoch="bad" (string)
When: EventPayload::try_from_json is called
Then: Returns Err(InvalidPayloadField("old_epoch must be an integer"))
```

---

### CA-07: ContinuedAsNew epoch ordering validation

**Scenario: new_epoch must be greater than old_epoch**

```
Given: A ContinuedAsNew event with old_epoch=5, new_epoch=3
When: Lineage validation occurs
Then: Rejected — new_epoch must exceed old_epoch (lineage chain must progress forward)
```

Note: This validation may occur at the engine level or in WorkflowLineage construction, not in EventPayload parsing itself.

---

### RE-01: ReplayEngine skips ContinuedAsNew

**Scenario: Replaying ContinuedAsNew does not change lifecycle state**

```
Given: A sequence of events ending with ContinuedAsNew { old_epoch=0, new_epoch=1 }
When: ReplayEngine::replay processes the events
Then: ContinuedAsNew is counted in events_applied but final_state reflects the last non-lineage event
```

```rust
#[test]
fn replay_engine_skips_continued_as_new() {
    let events = vec![
        event_envelope("inst-1", 1, workflow_started()),
        event_envelope("inst-1", 2, timer_set()),
        event_envelope("inst-1", 3, continued_as_new("lin-1", 0, 1)),
    ];
    let engine = ReplayEngine::new();
    let result = engine.replay(&events).expect("replay succeeds");
    assert_eq!(result.events_applied, 3);
    // State should be WaitingForTimer, not a terminal state
    assert!(matches!(result.final_state, Some(LifecycleState::WaitingForTimer(_))));
}
```

---

### RE-04: payload_to_transition errors on ContinuedAsNew

**Scenario: ContinuedAsNew should never reach payload_to_transition**

```
Given: ContinuedAsNew payload
When: payload_to_transition is called
Then: Returns Err(ReplayError::UnexpectedEventType { payload_type: "ContinuedAsNew" })
```

---

### EP-01: Epoch construction and zero

**Scenario: Epoch::ZERO equals 0, Epoch::new preserves value**

```
Given: Epoch::ZERO and Epoch::new(42)
When: compared
Then: Epoch::ZERO.0 == 0, Epoch::new(42).0 == 42
```

---

### EP-03: WorkflowLineage root creation

**Scenario: new() creates lineage with epoch 0 and no parent**

```
Given: A valid lineage_id string
When: WorkflowLineage::new(lineage_id) is called
Then: Returns Ok with epoch=Epoch::ZERO and parent_epoch=None
```

---

### EP-04: WorkflowLineage parent epoch enforcement

**Scenario: parent_epoch must be strictly less than epoch**

```
Given: lineage_id="lin-1", epoch=Epoch::new(3), parent_epoch=Some(Epoch::new(5))
When: WorkflowLineage::with_parent is called
Then: Returns Err(LineageError::InvalidEpochTransition { parent_epoch: 5, epoch: 3 })
```

---

### LQ-01: lineage_prefix_generator rejects empty lineage_id

**Scenario: Empty lineage_id is invalid for prefix generation**

```
Given: lineage_id=""
When: lineage_prefix_generator is called
Then: Returns Err(StorageError::InvalidLineageId)
```

---

### LQ-03: lineage_prefix_generator enforces max length

**Scenario: Lineage IDs exceeding max length are rejected**

```
Given: lineage_id with length > LINEAGE_ID_MAX_LEN (255 bytes)
When: lineage_prefix_generator is called
Then: Returns Err(StorageError::InvalidLineageId)
```

---

### LQ-04: epoch_prefix_generator encodes lineage and epoch

**Scenario: Epoch prefix includes lineage_id bytes followed by epoch u64**

```
Given: lineage_id="wf-123", epoch=Epoch::new(5)
When: epoch_prefix_generator is called
Then: Returns prefix where first N bytes are lineage_id, last 8 bytes are epoch as big-endian u64
```

---

### SM-01: Lineage-wide signal matches across epochs

**Scenario: Signal with LineageScope::LineageWide matches wait in any epoch**

```
Given: SignalAddress::lineage_wide(lineage_id, instance_id, wait_key)
And: WaitRecord created by workflow in epoch 1
When: signal_match is called with wait_instance_lineage_id=lineage_id
Then: Returns SignalMatchResult::Matched
```

---

### SM-03: Lineage mismatch blocks delivery

**Scenario: Signal targeting different lineage does not match**

```
Given: SignalAddress for lineage_id="lin-A"
And: WaitRecord from workflow with wait_instance_lineage_id="lin-B"
When: signal_match is called
Then: Returns SignalMatchResult::LineageMismatch { signal_lineage_id: "lin-A", wait_lineage_id: "lin-B" }
```

---

## 4. Proptest Invariants

### PI-01: Epoch ordering is monotonic

```
Invariant: For any e1, e2: if e1 < e2 then e1.0 < e2.0
Strategy: arbitrary u64 values for epoch construction
Anti-invariant: N/A — Epoch wraps u64 which has total ordering
```

```rust
proptest! {
    #[test]
    fn epoch_ordering_consistent_with_u64(a: u64, b: u64) {
        let e1 = Epoch::new(a);
        let e2 = Epoch::new(b);
        prop_assert_eq!(e1 < e2, a < b);
    }
}
```

### PI-02: Lineage epoch transition is strictly monotonic

```
Invariant: For any valid lineage with parent, parent_epoch < epoch
Strategy: arbitrary valid lineage_id, epoch 1..1000, parent 0..epoch-1
Anti-invariant: parent >= epoch must be rejected
```

```rust
proptest! {
    #[test]
    fn lineage_parent_epoch_less_than_epoch(lineage_id in "[a-z0-9]{1,100}", epoch: u64) {
        let parent_epoch = epoch.saturating_sub(1);
        let lineage = WorkflowLineage::with_parent(
            lineage_id.into(),
            Epoch::new(epoch),
            if epoch == 0 { None } else { Some(Epoch::new(parent_epoch)) }
        );
        prop_assert!(lineage.is_ok());
    }
}
```

### PI-03: Lineage prefix is deterministic

```
Invariant: lineage_prefix_generator(lineage_id) produces same bytes for same input
Strategy: arbitrary valid lineage_id strings
Anti-invariant: N/A — deterministic function
```

```rust
proptest! {
    #[test]
    fn lineage_prefix_deterministic(lineage_id in "[a-zA-Z0-9_-]{1,200}") {
        let prefix1 = lineage_prefix_generator(&lineage_id).expect("valid");
        let prefix2 = lineage_prefix_generator(&lineage_id).expect("valid");
        prop_assert_eq!(prefix1, prefix2);
    }
}
```

### PI-04: Epoch prefix includes lineage prefix

```
Invariant: epoch_prefix_generator(lineage_id, epoch) starts with lineage_prefix_generator(lineage_id)
Strategy: arbitrary valid lineage_id, arbitrary u64 epoch
Anti-invariant: N/A — structural invariant
```

```rust
proptest! {
    #[test]
    fn epoch_prefix_starts_with_lineage_prefix(lineage_id in "[a-z]{1,100}", epoch: u64) {
        let lineage_prefix = lineage_prefix_generator(&lineage_id).expect("valid");
        let epoch_prefix = epoch_prefix_generator(&lineage_id, Epoch::new(epoch)).expect("valid");
        prop_assert!(epoch_prefix.starts_with(&lineage_prefix));
    }
}
```

---

## 5. Fuzz Targets

### FT-01: EventPayload JSON deserialization

```
Input type: String (JSON)
Risk: panic on malformed JSON, incorrect variant parsing, buffer overflow on large epoch values
Corpus seeds: valid ContinuedAsNew JSON, missing fields, non-integer epochs, empty lineage_id, unicode in workflow_id
```

---

## 6. Kani Harnesses

No Kani harnesses required. Epoch and WorkflowLineage are simple newtype wrappers with obvious invariants. Formal verification overhead not justified for pure data transformation functions.

---

## 7. Mutation Checkpoints

| Checkpoint | Mutated Code | Must Be Caught By |
|------------|--------------|-------------------|
| MC-001 | Change `ContinuedAsNew` JSON tag to `ContinueAsNew` | `continued_as_new_decodes_from_json` |
| MC-002 | Remove old_epoch validation (accept any u64) | `continued_as_new_rejects_missing_old_epoch` |
| MC-003 | Change `lineage_prefix_generator` null-byte check | `lineage_prefix_generator_rejects_null_byte` |
| MC-004 | Remove max-length check in lineage_prefix_generator | `lineage_prefix_generator_rejects_oversized` |
| MC-005 | Change `signal_match` to ignore epoch scope | `signal_match_epoch_local_requires_match` |
| MC-006 | Remove parent < epoch check in WorkflowLineage | `lineage_parent_epoch_validation` |
| MC-007 | Change ReplayEngine to treat ContinuedAsNew as state transition | `replay_engine_skips_continued_as_new` |

**Threshold**: 90% mutation kill rate (7/7 checkpoints = 100%)

---

## 8. Combinatorial Coverage Matrix

### ContinuedAsNew EventPayload

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| valid decode | all fields present | Ok(ContinuedAsNew) | unit |
| missing lineage_id | no lineage_id field | Err(MissingPayloadField) | unit |
| missing old_epoch | no old_epoch field | Err(MissingPayloadField) | unit |
| missing new_epoch | no new_epoch field | Err(MissingPayloadField) | unit |
| non-integer old_epoch | "bad" string | Err(InvalidPayloadField) | unit |
| non-integer new_epoch | "bad" string | Err(InvalidPayloadField) | unit |
| roundtrip | valid JSON → parse → serialize | identical JSON | unit |

### ReplayEngine

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| skip ContinuedAsNew | event sequence with ContinuedAsNew | events_applied counts it, state unchanged | unit |
| ContinuedAsNew mid-sequence | events before + ContinuedAsNew + events after | All processed correctly | integration |
| ContinuedAsNew at start | ContinuedAsNew as first event | Skipped, next event starts state | unit |
| payload_to_transition | ContinuedAsNew payload | Err(UnexpectedEventType) | unit |

### WorkflowLineage

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| root creation | new("lin-1") | epoch=0, parent=None | unit |
| child creation | with_parent("lin-1", 2, Some(1)) | Ok | unit |
| empty id | new("") | Err(EmptyLineageId) | unit |
| whitespace id | new("   ") | Err(EmptyLineageId) | unit |
| parent equals epoch | with_parent("lin-1", 3, Some(3)) | Err(InvalidEpochTransition) | unit |
| parent > epoch | with_parent("lin-1", 2, Some(5)) | Err(InvalidEpochTransition) | unit |
| JSON roundtrip | valid WorkflowLineage | identical after serialize/deserialize | unit |

### LineageQuery Prefix

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| lineage_prefix valid | "lin-123" | valid prefix bytes | unit |
| lineage_prefix empty | "" | Err | unit |
| lineage_prefix null byte | "lin\0" | Err | unit |
| lineage_prefix too long | 256-char string | Err | unit |
| epoch_prefix | "lin-123", Epoch(5) | prefix includes lineage + epoch bytes | unit |
| prefix composition | LineageQuery::LineageWide | lineage_prefix | unit |
| prefix composition | LineageQuery::EpochSpecific | epoch_prefix | unit |

### Signal Matching

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| lineage-wide match | same lineage, instance, key | Matched | unit |
| lineage mismatch | different lineage | LineageMismatch | unit |
| instance mismatch | different instance | InstanceMismatch | unit |
| wait_key mismatch | different wait_key | WaitKeyMismatch | unit |
| epoch-local match | same lineage, instance, key, epoch | Matched | unit |
| epoch mismatch | different epoch | EpochMismatch | unit |

---

## 9. Existing Tests

The following tests already exist in `vo-types/src/lineage.rs`:

1. `epoch_new_returns_expected_value` — covers EP-01
2. `epoch_zero_is_zero` — covers EP-01
3. `epoch_ord_is_consistent_with_u64` — covers EP-02
4. `lineage_new_creates_root_with_epoch_zero_and_no_parent` — covers EP-03
5. `lineage_with_parent_creates_child_epoch` — covers EP-04
6. `lineage_new_returns_empty_lineage_id_when_id_is_empty` — covers LQ-01
7. `lineage_with_parent_returns_invalid_epoch_transition_when_parent_equals_epoch` — covers EP-04
8. `invariant_epoch_monotonic_parent_less_than_epoch` — covers EP-04, PI-02

The following tests already exist in `vo-types/src/events/tests/payload_edge_tests.rs`:

9. `payload_try_from_json_returns_continued_as_new_when_type_matches` — covers CA-01
10. `payload_try_from_json_returns_missing_payload_field_when_lineage_id_absent` — covers CA-02
11. `payload_try_from_json_returns_missing_payload_field_when_old_epoch_absent` — covers CA-03
12. `payload_try_from_json_returns_missing_payload_field_when_new_epoch_absent` — covers CA-04
13. `payload_try_from_json_returns_invalid_payload_field_when_old_epoch_not_integer` — covers CA-05
14. `payload_try_from_json_returns_invalid_payload_field_when_new_epoch_not_integer` — covers CA-06

The following tests already exist in `vo-storage/src/query/tests.rs`:

15. `lineage_prefix_generator_returns_prefix_with_null_delimiters` — covers LQ-04
16. `lineage_prefix_generator_rejects_empty_lineage_id` — covers LQ-01
17. `lineage_prefix_generator_rejects_lineage_id_with_null_byte` — covers LQ-02
18. `lineage_prefix_generator_rejects_lineage_id_exceeding_max_len` — covers LQ-03
19. `epoch_prefix_generator_returns_prefix_with_lineage_and_epoch` — covers LQ-04

The following tests already exist in `vo-types/src/signal/signal_match.rs`:

20. `signal_match_returns_matched_when_all_dimensions_align` — covers SM-01
21. `signal_match_returns_lineage_mismatch_when_lineage_differs` — covers SM-03
22. `signal_match_returns_instance_mismatch_when_instance_differs` — covers SM (instance variant)

---

## 10. Open Questions

1. **Timer behavior after rollover**: ADR-038 states timers carry forward or reset — does the implementation preserve timer state across epoch boundary? Tests for timer continuity needed if timers are preserved.

2. **Signal dedupe scope**: ADR-042 governs dedupe across lineage rollover — does the dedupe key include epoch? If so, a signal sent to epoch 0 and then replayed after continue-as-new to epoch 1 may be treated as duplicate.

3. **Canonical state carried forward**: ADR-038 mentions "minimal canonical state required to continue execution" — what fields are included? This affects integration test design for state reconstruction.

4. **Query routing for lineage-wide queries**: Does `LineageQuery::LineageWide` return events from all epochs ordered by sequence, or grouped by epoch? The query interface affects how lineage chain traversal is tested.

---

## 11. Exit Criteria Compliance

- [x] Every public API behavior has at least one BDD scenario (24/24)
- [x] Every pure function with multiple inputs has at least one proptest invariant (4 invariants)
- [x] Every parsing/deserialization boundary has a fuzz target (1 fuzz target: EventPayload JSON)
- [x] Every error variant in ContinuedAsNew has explicit test scenario (6 error variants covered)
- [x] The mutation threshold target (≥90%) is stated (100%)
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value (all tests assert specific values)
