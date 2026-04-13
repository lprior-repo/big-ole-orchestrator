# Test Plan: Replay Engine (ADR-027)

**Contract**: `docs/adr/v2/ADR-027-v2-deterministic-event-sourced-replay.md`
**Issue**: ve-k9pg
**Target crates**: `crates/vo-core/src/replay/` (replay engine), `crates/vo-storage/src/query/` (storage integration)
**Bead**: ve-k9pg

## Scope

This plan covers exhaustive testing for the deterministic replay engine — the pure function engine that reads a slice of `EventEnvelope`s and reduces them into a `LifecycleState` without side effects. It addresses: deterministic replay from event log, event ordering guarantees, snapshot+replay recovery, crash injection at every transition point, concurrent replay conflicts, proptest invariants for state machine transitions, and fuzz targets for event deserialization.

Tests are organized by the Testing Trophy: unit tests (pure state machine + happy/error paths), targeted property tests via proptest, integration tests for storage layer, and fuzz targets for deserialization boundaries.

---

## 1. Behavior Inventory

### ReplayEngine Core Behaviors

| ID | Behavior |
|----|----------|
| B-001 | `ReplayEngine::new()` creates a stateless instance |
| B-002 | Empty event list returns `final_state: None, events_applied: 0` |
| B-003 | `WorkflowStarted` maps to `AssignToNode` transition (Pending → RunningDecision) |
| B-004 | `StepScheduled` maps to `StepScheduled` transition (RunningDecision → StepScheduled) |
| B-005 | `StepStarted` maps to `ExecuteStep` transition (StepScheduled → StepExecuting) |
| B-006 | `StepCompleted` maps to `CompleteStep` transition (StepExecuting → Completed) |
| B-007 | `StepFailed` maps to `Fail` transition (StepExecuting → Failed) |
| B-008 | `TimerSet` maps to `WaitForTimer` transition (StepExecuting → WaitingForTimer) |
| B-009 | `TimerFired` maps to `TimerFired` transition (WaitingForTimer → StepExecuting) |
| B-010 | `TimerExpired` maps to `Fail` transition (WaitingForTimer → Failed) |
| B-011 | `WorkflowCancelled` maps to `Cancel` transition (Pending → Cancelled) |
| B-012 | `CancelRequested` maps to `Cancel` transition (any non-terminal → Cancelled) |
| B-013 | `WorkflowFailed` maps to `Fail` transition (RunningDecision → Failed) |
| B-014 | `InstanceResumed` maps to `InstanceResumed` transition (Failed → RunningDecision) |
| B-015 | `ContinuedAsNew` is counted as applied but produces no state change (no-op) |
| B-016 | Replay stops processing after `Completed` or `Cancelled` (terminal states) |
| B-017 | Replay continues after `Failed` if followed by `InstanceResumed` |
| B-018 | Replay is deterministic: same events always produce same result |

### Validation Behaviors

| ID | Behavior |
|----|----------|
| B-019 | Events with different `instance_id` return `ReplayError::InstanceMismatch` |
| B-020 | Non-contiguous sequence numbers return `ReplayError::SequenceGap` |
| B-021 | Duplicate sequence numbers return `ReplayError::SequenceDuplicate` |
| B-022 | Malformed JSON payload returns `ReplayError::PayloadDecodeFailed` |
| B-023 | Invalid state transition returns `ReplayError::TransitionFailed` |
| B-024 | Unknown event type returns `ReplayError::UnexpectedEventType` |
| B-025 | Upcasting failure returns `ReplayError::UpcastingFailed` |

### Snapshot + Replay Behaviors (ADR-027 §7)

| ID | Behavior |
|----|----------|
| B-026 | Replay post-snapshot events reconstructs correct state |
| B-027 | Snapshot at sequence N + events N+1..M produces same result as 1..M |
| B-028 | `StepScheduled`/`StepStarted` without `StepCompleted` → rerun under new fence |
| B-029 | `EffectPrepared` without `EffectCommitted` → reconcile via connector |
| B-030 | `WaitingForTimer` → re-register timer using recorded event |
| B-031 | `WaitingForSignal` → re-register signal wait using deterministic wake-up rules |
| B-032 | `Compensating::*` → replay compensation planning state |

### Ordering Guarantees (ADR-027 §6)

| ID | Behavior |
|----|----------|
| B-033 | Sequence validation enforces strict ascending order |
| B-034 | First event starts from `Pending` (not `None`) |
| B-035 | Subsequent events use accumulated state |
| B-036 | Events are applied in array order (not sorted) |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| Unit (Calc) | 85 | Pure state machine + all error variants; exhaustive combinatorial |
| Integration | 22 | Storage layer replay with real fjall keyspace |
| Property (proptest) | 18 | Invariants across all state/event combinations |
| Fuzz | 6 | Deserialization boundaries at payload parsing |
| Kani | 4 | Critical invariants: determinism, no panic, overflow safety |
| **Total** | **135** | |

**Rationale**: The replay engine is a pure function — the majority of tests are unit tests validating the state machine. Integration tests exercise the storage boundary. Property tests cover combinatorial explosion. Fuzz targets cover the deserialization attack surface.

---

## 3. BDD Scenarios

### 3.1 Empty Replay

```
### Behavior: Empty event list returns identity result
Given: ReplayEngine and empty event list
When: replay() is called
Then: Returns Ok(ReplayResult { final_state: None, events_applied: 0 })
```

### 3.2 Happy Path: Complete Workflow Lifecycle

```
### Behavior: Complete pure step lifecycle reaches Completed
Given: ReplayEngine and events [WorkflowStarted, StepScheduled, StepStarted, StepCompleted]
When: replay() is called
Then: Returns Ok(ReplayResult { final_state: Some(Completed), events_applied: 4 })

### Behavior: Complete lifecycle with timer reaches Completed
Given: ReplayEngine and events [WorkflowStarted, StepScheduled, StepStarted, TimerSet, TimerFired, StepCompleted]
When: replay() is called
Then: Returns Ok(ReplayResult { final_state: Some(Completed), events_applied: 6 })

### Behavior: Failed workflow can be resumed
Given: ReplayEngine and events [WorkflowStarted, StepScheduled, StepStarted, StepFailed, InstanceResumed, StepScheduled]
When: replay() is called
Then: Returns Ok(ReplayResult { final_state: Some(StepScheduled), events_applied: 6 })

### Behavior: Workflow can be cancelled at any non-terminal state
Given: ReplayEngine at state X and CancelRequested event
When: replay() is called  
Then: Returns Ok(ReplayResult { final_state: Some(Cancelled), events_applied: ... })
And: X can be Pending, RunningDecision, StepScheduled, StepExecuting, WaitingForTimer
```

### 3.3 Validation Errors

```
### Behavior: InstanceMismatch when instance_ids differ
Given: ReplayEngine and events with different instance_ids
When: replay() is called
Then: Returns Err(ReplayError::InstanceMismatch { expected, actual })

### Behavior: SequenceGap when sequences are non-contiguous
Given: ReplayEngine and events [seq=1, seq=3]
When: replay() is called
Then: Returns Err(ReplayError::SequenceGap { expected: 2, actual: 3, at_index: 1 })

### Behavior: SequenceDuplicate when sequence repeats
Given: ReplayEngine and events [seq=1, seq=1]
When: replay() is called
Then: Returns Err(ReplayError::SequenceDuplicate { sequence: 1, first_at_index: 0, second_at_index: 1 })

### Behavior: PayloadDecodeFailed on malformed JSON
Given: ReplayEngine and event with invalid JSON payload
When: replay() is called
Then: Returns Err(ReplayError::PayloadDecodeFailed { sequence: 1, source: ... })

### Behavior: UnexpectedEventType on ContinuedAsNew in payload_to_transition
Given: ReplayEngine and event with ContinuedAsNew payload (reaching payload_to_transition)
When: replay() is called
Then: Returns Err(ReplayError::UnexpectedEventType { payload_type: "ContinuedAsNew", sequence: ... })
```

### 3.4 ContinuedAsNew No-op

```
### Behavior: ContinuedAsNew counts as applied but does not change state
Given: ReplayEngine and events [..., ContinuedAsNew, ...]
When: replay() is called
Then: events_applied includes ContinuedAsNew count
And: final_state is unchanged by ContinuedAsNew

### Behavior: ContinuedAsNew at terminal state is ignored
Given: ReplayEngine and events [...Completed, ContinuedAsNew]
When: replay() is called
Then: final_state is Completed with events_applied stopping before ContinuedAsNew
```

### 3.5 Terminal State Handling

```
### Behavior: Events after Completed are ignored
Given: ReplayEngine and events [...StepCompleted, InstanceResumed]
When: replay() is called
Then: final_state is Completed, events_applied stops at StepCompleted

### Behavior: Events after Cancelled are ignored
Given: ReplayEngine and events [...CancelRequested, any-event]
When: replay() is called
Then: final_state is Cancelled, events_applied stops at CancelRequested

### Behavior: InstanceResumed after Failed recovers workflow
Given: ReplayEngine and events [...StepFailed, InstanceResumed, ...]
When: replay() is called
Then: state transitions from Failed to RunningDecision
And: replay continues processing subsequent events

### Behavior: Non-InstanceResumed events after Failed are rejected
Given: ReplayEngine and events [...StepFailed, StepScheduled]
When: replay() is called
Then: Returns Err(ReplayError::TransitionFailed { state: Failed, ... })
```

### 3.6 Upcaster Integration

```
### Behavior: replay_with_upcaster upcasts before replay
Given: ReplayEngine, UpcasterRegistry, and v0 events
When: replay_with_upcaster() is called
Then: Each envelope is upcast via registry.upcast_envelope before replay
And: Errors return ReplayError::UpcastingFailed

### Behavior: Empty events with upcaster returns empty result
Given: ReplayEngine, UpcasterRegistry, and empty events
When: replay_with_upcaster() is called
Then: Returns Ok(ReplayResult { final_state: None, events_applied: 0 })
```

### 3.7 Snapshot + Recovery (ADR-027 §7)

```
### Behavior: Replay post-snapshot events reconstructs correct final state
Given: Snapshot at sequence S and events S+1..N
When: replay() is called with post-snapshot events
Then: final_state matches result of replaying events 1..N

### Behavior: StepScheduled/StepStarted without completion is detected
Given: Events [...StepScheduled] or [...StepStarted] without StepCompleted/StepFailed
When: replay() completes
Then: final_state is StepScheduled or StepExecuting respectively
And: Recovery logic can determine this is a rerunnable state

### Behavior: EffectPrepared without EffectCommitted is detected
Given: Events [...EffectPrepared] without EffectCommitted
When: replay() completes  
Then: final_state reflects EffectPrepared state
And: Recovery logic can reconcile via effect_id
```

### 3.8 Ordering Guarantees

```
### Behavior: Sequence validation enforces strict ascending order
Given: Events [seq=1, seq=3, seq=2]
When: replay() is called
Then: Returns Err(ReplayError::SequenceGap { expected: 2, actual: 3, at_index: 1 })

### Behavior: First event starts from Pending
Given: Events starting at arbitrary sequence N
When: replay() is called
Then: First event transitions from Pending (not None)

### Behavior: Events are processed in array order
Given: Events with valid sequences in specific order
When: replay() is called
Then: State transitions follow exact array order, not sorted order
```

---

## 4. Proptest Invariants

### 4.1 Determinism Invariants

```
### Proptest: replay_determinism
Invariant: For any valid event sequence, replay() called twice produces identical ReplayResult
Strategy: prop_strategy based on valid event sequence generation
Anti-invariant: None — valid sequences always deterministic
```

```
### Proptest: events_applied_never_exceeds_input_len
Invariant: result.events_applied <= events.len() for all inputs
Strategy: events with arbitrary sequence numbers
Anti-invariant: N/A — invariant holds universally
```

```
### Proptest: empty_replay_preserves_invariants
Invariant: replay(&[]) returns (None, 0) always
Strategy: No inputs needed
Anti-invariant: N/A
```

### 4.2 State Machine Transition Invariants

```
### Proptest: all_valid_transitions_accepted
Invariant: For any (LifecycleState, TransitionEvent) where transition is valid per get_valid_transitions(), apply() returns Ok(new_state)
Strategy: Generate all (state, event) pairs from LifecycleState::all_variants() and TransitionEvent::all_variants()
Anti-invariant: Invalid pairs should return Err
```

```
### Proptest: terminal_states_reject_all_transitions
Invariant: For any terminal state (Completed, Failed, Cancelled) and any non-InstanceResumed event, apply() returns Err(TerminalStateTransition)
Strategy: All terminal states × all non-InstanceResumed events
Anti-invariant: InstanceResumed from Failed is valid (not terminal-transition)
```

```
### Proptest: failed_accepts_only_instance_resumed
Invariant: apply(Failed, InstanceResumed) = Ok(RunningDecision); apply(Failed, any-other) = Err
Strategy: Failed × all TransitionEvent variants
Anti-invariant: All non-InstanceResumed events from Failed
```

### 4.3 Sequence Validation Invariants

```
### Proptest: sequence_gap_detected_at_correct_position
Invariant: For events with gap at index i, ReplayError::SequenceGap contains at_index: i
Strategy: Generate events with known gap positions
Anti-invariant: N/A — gap detection is exact
```

```
### Proptest: sequence_duplicate_detected_at_correct_position
Invariant: For events with duplicate at indices i and j (i < j), ReplayError::SequenceDuplicate contains first_at_index: i, second_at_index: j
Strategy: Generate events with known duplicate positions
Anti-invariant: N/A
```

```
### Proptest: instance_mismatch_detects_first_differing_pair
Invariant: For events with instance_ids differing at index i, ReplayError::InstanceMismatch contains expected: events[0].instance_id
Strategy: Generate events with known mismatch positions
Anti-invariant: N/A
```

### 4.4 Payload Mapping Invariants

```
### Proptest: all_event_payloads_map_to_valid_transitions
Invariant: For all EventPayload variants that are not ContinuedAsNew, payload_to_transition() returns Ok(TransitionEvent)
Strategy: All EventPayload variants
Anti-invariant: ContinuedAsNew returns Err(UnexpectedEventType) — this is expected behavior
```

### 4.5 Recovery Invariants

```
### Proptest: snapshot_replay_equivalence
Invariant: replay(snapshot_events ++ post_snapshot_events) = replay(all_events)
Strategy: Generate arbitrary sequence, split at random point
Anti-invariant: N/A
```

---

## 5. Fuzz Targets

### 5.1 EventEnvelope Deserialization

```
### Fuzz Target: replay_with_malformed_json_payload
Input type: arbitrary bytes
Risk: Panic (unwrap in JSON parsing), OOM (large allocation), logic error (wrong state)
Corpus seeds: valid JSON payloads, truncated JSON, empty bytes, non-UTF8, deeply nested objects
```

```
### Fuzz Target: replay_with_unknown_event_type
Input type: JSON bytes with "type" field set to arbitrary string
Risk: Logic error where unknown type slips through
Corpus seeds: Valid type strings, empty string, numbers, arrays, unicode
```

### 5.2 Sequence Number Handling

```
### Fuzz Target: replay_with_extreme_sequence_numbers
Input type: u64 sequence values
Risk: Overflow in sequence arithmetic (expected + 1), wraparound
Corpus seeds: 0, 1, u64::MAX, u64::MAX - 1, powers of 2
```

### 5.3 Instance ID Handling

```
### Fuzz Target: replay_with_extreme_instance_ids
Input type: arbitrary string instance_id values
Risk: Empty string, very long strings, unicode, special characters
Corpus seeds: Empty string, ASCII, unicode, 1KB string, 1MB string
```

### 5.4 Timestamp Handling

```
### Fuzz Target: replay_with_extreme_timestamps
Input type: u64 timestamp_ms values
Risk: Zero, u64::MAX, non-monotonic (but timestamp is not used in replay decision per ADR-027)
Corpus seeds: 0, u64::MAX, arbitrary values
```

### 5.5 Schema Version Handling

```
### Fuzz Target: replay_with_extreme_schema_versions
Input type: u8 schema_version values  
Risk: Version boundary issues, upcaster lookup failures
Corpus seeds: 0, 1, u8::MAX, values that may exceed registered upcasters
```

### 5.6 Combined Payload Parsing

```
### Fuzz Target: replay_with_combined_payload_edges
Input type: Complete EventEnvelope as JSON
Risk: Interaction effects between fields, missing required fields, extra fields
Corpus seeds: Valid complete envelopes, missing type, missing workflow_id, extra fields, type confusion
```

---

## 6. Kani Harnesses

### 6.1 Determinism Proof

```
### Kani Harness: kani_replay_determinism
Property: For any valid event sequence, replay() is called twice, both calls return equal ReplayResult
Bound: Single event with arbitrary sequence number (1..10 to limit state space)
Rationale: Formal proof of determinism — proptest can only show repeated success, not prove it
```

### 6.2 No Panic Proof

```
### Kani Harness: kani_replay_never_panics
Property: For any valid EventEnvelope with sequence >= 1, replay() never panics
Bound: Single event with bounded sequence (1..1000 to limit explore)
Rationale: ADR-027 guarantees deterministic replay — panics break the contract
```

### 6.3 Transition Exhaustiveness

```
### Kani Harness: kani_all_transitions_covered
Property: For every (LifecycleState, TransitionEvent) pair, either apply() returns Ok or Err(TerminalStateTransition|InvalidTransition)
Bound: All 6 LifecycleState × 10 TransitionEvent = 60 pairs
Rationale: State machine must be exhaustive — missing cases would allow invalid states
```

### 6.4 Overflow Safety

```
### Kani Harness: kani_sequence_arithmetic_no_overflow  
Property: In replay() with events.len() = N, expected_seq computation never overflows u64
Bound: Events with sequence numbers in [0, u64::MAX]  
Rationale: Sequence validation performs expected + 1 — must be bounded-safe
```

---

## 7. Mutation Checkpoints

**Threshold**: ≥90% mutation kill rate

### Critical Mutations

| Location | Mutation | Must Be Caught By |
|----------|----------|-------------------|
| `engine.rs:39` | Remove empty check | `empty_replay_returns_none_state` |
| `engine.rs:47` | Remove instance_id check | `instance_mismatch_detected` |
| `engine.rs:58` | Remove sequence validation | `sequence_gap_detected`, `sequence_duplicate_detected` |
| `engine.rs:64` | Change `==` to `>=` in duplicate check | `sequence_duplicate_detected` |
| `engine.rs:71` | Change `!=` to `==` in gap check | `sequence_gap_detected` |
| `engine.rs:95` | Remove ContinuedAsNew early return | `continued_as_new_counts_as_applied` |
| `engine.rs:103` | Change `unwrap_or(Pending)` to `unwrap()` | `first_event_starts_from_pending` |
| `engine.rs:112` | Add `Failed` to terminal states | `failed_accepts_instance_resumed` |
| `transition.rs:113` | Remove terminal state rejection | `terminal_states_reject_all` |

---

## 8. Combinatorial Coverage Matrix

### 8.1 LifecycleState × TransitionEvent Matrix

| State | Valid Transitions | Invalid Transitions |
|-------|-------------------|---------------------|
| Pending | AssignToNode, Cancel | All others |
| RunningDecision | StepScheduled, Fail, Cancel | AssignToNode, ExecuteStep |
| StepScheduled | ExecuteStep, Fail, Cancel | AssignToNode, StepScheduled |
| StepExecuting | WaitForTimer, CompleteStep, Fail, Cancel | AssignToNode, ExecuteStep |
| WaitingForTimer | TimerFired, TimerExpired, Cancel, Fail | ExecuteStep, CompleteStep |
| Completed | (none — terminal) | All |
| Failed | InstanceResumed | All except InstanceResumed |
| Cancelled | (none — terminal) | All |

### 8.2 EventPayload × LifecycleState Coverage

| EventPayload | From State | To State | Test ID |
|--------------|------------|----------|---------|
| WorkflowStarted | Pending | RunningDecision | B-003 |
| StepScheduled | RunningDecision | StepScheduled | B-004 |
| StepStarted | StepScheduled | StepExecuting | B-005 |
| StepCompleted | StepExecuting | Completed | B-006 |
| StepFailed | StepExecuting | Failed | B-007 |
| TimerSet | StepExecuting | WaitingForTimer | B-008 |
| TimerFired | WaitingForTimer | StepExecuting | B-009 |
| TimerExpired | WaitingForTimer | Failed | B-010 |
| WorkflowCancelled | Pending | Cancelled | B-011 |
| CancelRequested | Any non-terminal | Cancelled | B-012 |
| WorkflowFailed | RunningDecision | Failed | B-013 |
| InstanceResumed | Failed | RunningDecision | B-014 |
| ContinuedAsNew | Any | (no change) | B-015 |

### 8.3 Error Variant Coverage

| ReplayError Variant | Trigger Scenario | Test Layer |
|---------------------|------------------|------------|
| InstanceMismatch | Events with different instance_ids | unit |
| SequenceGap | Non-contiguous sequences | unit |
| SequenceDuplicate | Same sequence twice | unit |
| PayloadDecodeFailed | Malformed JSON | fuzz + unit |
| TransitionFailed | Invalid state transition | unit |
| UnexpectedEventType | ContinuedAsNew reaching payload_to_transition | unit |
| UpcastingFailed | Upcaster registry failure | unit |

---

## 9. Integration Tests (Storage Layer)

| ID | Test | Category | Expected |
|----|------|----------|----------|
| IT-001 | Empty keyspace returns empty iterator | Integration | Vec is empty |
| IT-002 | Single event returned in order | Integration | Vec.len() = 1 |
| IT-003 | Multiple events returned in sequence order | Integration | Vec.len() = N, ordered |
| IT-004 | Sequence gap detected during iteration | Integration | Err(StorageError::SequenceGap) |
| IT-005 | Corrupt payload returns error | Integration | Err(StorageError::CorruptEventPayload) |
| IT-006 | Unsupported version returns error | Integration | Err(StorageError::UnsupportedVersion) |
| IT-007 | Different instances are isolated | Integration | Each instance has own events |
| IT-008 | Iteration stops after first error | Integration | Error terminates iterator |
| IT-009 | Non-one starting sequence accepted | Integration | Valid replay from seq 10+ |
| IT-010 | Gap at start detected | Integration | Err at index 1 |
| IT-011 | Large sequence range (1M+) | Integration | All events returned |
| IT-012 | Snapshot + replay end-to-end | Integration | Same result as full replay |

---

## 10. Edge Case Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| ED-001 | Empty event list | Edge | final_state: None |
| ED-002 | Single event | Edge | final_state after 1 apply |
| ED-003 | Sequence number 0 | Boundary | Valid, starts from Pending |
| ED-004 | Sequence number u64::MAX | Boundary | No overflow in arithmetic |
| ED-005 | Empty instance_id | Edge | Valid, no validation failure |
| ED-006 | Arbitrary starting sequence (100, 500) | Boundary | Accepts any first seq |
| ED-007 | Multiple failure/recovery cycles | Edge | 3x Failed→Resumed cycle succeeds |
| ED-008 | ContinuedAsNew counted but no state change | Edge | events_applied includes CAN |
| ED-009 | ContinuedAsNew at terminal state ignored | Edge | Stops before CAN |
| ED-010 | Mixed instance_ids detected at first difference | Edge | InstanceMismatch at index 1 |
| ED-011 | Double InstanceResumed rejected | Edge | TransitionFailed at second |
| ED-012 | Workflow cancelled from various states | Edge | Cancelled from all non-terminal |

---

## 11. Test File Organization

```
crates/vo-core/src/replay/
  mod.rs                              # Module root
  engine.rs                           # ReplayEngine (existing)
  types.rs                            # ReplayResult, ReplayError (existing)
  test_helpers.rs                     # Shared test helpers (existing)
  tests.rs                            # Happy-path tests (existing, 13 behaviors)
  error_tests.rs                      # Error-path tests (existing)
  upcaster_tests.rs                   # Upcaster integration tests (existing)
  red_queen_adversarial_tests.rs      # Adversarial tests (existing, ~30 scenarios)
  kani_proptests.rs                   # Kani + proptest (existing)
  integration_tests.rs                # Snapshot + recovery tests (NEW)
  crash_injection_tests.rs            # Crash at every transition (NEW)
  concurrent_replay_tests.rs          # Concurrent replay conflicts (NEW)
  proptest_invariants.rs              # All proptest invariants (NEW)
  snapshot_recovery_tests.rs           # Snapshot + replay recovery (NEW)

crates/vo-storage/tests/
  integration_replay.rs               # Storage integration (existing)
  snapshot_replay_integration.rs      # Snapshot + replay integration (NEW)
```

---

## 12. Test Count Summary

| Category | Count |
|----------|-------|
| Happy-path behaviors (B-001 to B-018) | 18 |
| Validation errors (B-019 to B-025) | 7 |
| Snapshot + recovery (B-026 to B-032) | 7 |
| Ordering guarantees (B-033 to B-036) | 4 |
| BDD scenarios (Section 3) | 35 |
| Proptest invariants (Section 4) | 10 |
| Fuzz targets (Section 5) | 6 |
| Kani harnesses (Section 6) | 4 |
| Integration tests (Section 9) | 12 |
| Edge cases (Section 10) | 12 |
| **Total** | **115+** |

---

## Open Questions

1. **Concurrent replay conflicts**: The replay engine is stateless and pure — concurrent calls to `replay()` on different instances don't conflict. Should "concurrent replay conflicts" refer to concurrent replay of events for the same instance_id? If so, this requires a higher-level abstraction (the actor) not the engine itself.

2. **Snapshot format**: What is the persisted snapshot format? Is it a serialized `LifecycleState` with sequence number, or something more complex? This affects how `snapshot_recovery_tests` are structured.

3. **EffectPrepared/EffectCommitted events**: These events appear in ADR-027 §4 but don't appear in the current `EventPayload` enum or `payload_to_transition`. Should they be added, or are they handled externally?

4. **TimerExpired**: The `TimerExpired` event appears in `TransitionEvent` but I don't see a corresponding `EventPayload` variant. How is timer expiration modeled in the event log?
