# Test Plan: Event Sourcing Projection Engine

**Contract**: `docs/contracts/event-sourcing-projection-engine.md`
**Issue**: ve-68di
**Target crates**: `crates/vo-core/src/replay/` (replay engine), `crates/vo-storage/src/projection_compat/` (compat checking), `crates/vo-core/src/upcaster/` (upcaster registry)

## Scope

This plan covers exhaustive testing for the event sourcing projection engine — the system that transforms immutable event sequences into materialized read models. It addresses the `Projector` trait interface, the `ProjectionState` lifecycle state machine, all 16 invariants (INV-001 through INV-016), the full error taxonomy (`ProjectionError`, `ReplayError`, `ProjectionVersionError`, `ProjectionStateError`), the build/incremental/rebuild protocols, upcaster integration, checksummed storage integrity, and observability events. Tests are organized by the Testing Trophy: unit tests (majority), targeted property tests via proptest, and integration tests for cross-crate protocol sequences.

---

## 1. Projector Trait Interface Tests

### 1.1 Projector Trait Contract

| ID | Test | Category | Expected |
|----|------|----------|----------|
| PT-001 | `project(state, event)` is pure — same inputs produce identical output | INV-001 | Byte-identical state |
| PT-002 | `project(state, event)` has no side effects | INV-001 | No I/O, no external mutation |
| PT-003 | `initial_state()` returns zero-value state | INV-004 | All fields at default |
| PT-004 | `schema_version()` returns consistent value across calls | Correctness | Same u8 every time |
| PT-005 | Projector impl does not retain mutable state between calls | INV-003 | Second call unaffected by first |
| PT-006 | `ProjectionState: Clone` bound is satisfied | Trait bound | `state.clone()` compiles |
| PT-007 | `ProjectionState: Default` bound is satisfied | Trait bound | `ProjectionState::default()` compiles |
| PT-008 | `ProjectionState: Serialize` bound is satisfied | Trait bound | `serde_json::to_value(state)` compiles |

### 1.2 Projector Error Conversion

| ID | Test | Category | Expected |
|----|------|----------|----------|
| PE-001 | Projector error converts into `ProjectionError::Projector` | Error path | `Into::<ProjectionError>::into(err)` |
| PE-002 | Projector error message preserved through conversion | Correctness | Message string matches |

---

## 2. ProjectionState Lifecycle State Machine Tests

### 2.1 State Construction

| ID | Test | Category | Expected |
|----|------|----------|----------|
| LS-001 | `ProjectionState::Building` constructs without fields | Happy path | Variant matches |
| LS-002 | `ProjectionState::Ready` constructs | Happy path | Variant matches |
| LS-003 | `ProjectionState::Stale { detected_at, reason }` constructs | Happy path | Fields accessible |
| LS-004 | `ProjectionState::Rebuilding { progress, from_sequence }` constructs | Happy path | Fields accessible |
| LS-005 | `ProjectionState::Failed { reason, attempted_at }` constructs | Happy path | Fields accessible |

### 2.2 Valid State Transitions

| ID | Test | From | To | Trigger |
|----|------|------|----|---------|
| LT-001 | Building | Ready | Build completed successfully |
| LT-002 | Building | Failed | Build failed |
| LT-003 | Ready | Stale | Staleness detected (INV-011) |
| LT-004 | Stale | Rebuilding | Rebuild initiated (INV-012) |
| LT-005 | Rebuilding | Ready | Rebuild completed |
| LT-006 | Rebuilding | Failed | Rebuild failed |
| LT-007 | Failed | Building | Manual reset/retry |

### 2.3 Invalid State Transitions

| ID | Test | From | Attempted | Expected |
|----|------|------|-----------|----------|
| LI-001 | Failed | Ready | Err(TerminalStateTransition) |
| LI-002 | Ready | Rebuilding | Err(RebuildNotStale) |
| LI-003 | Building | Stale | Err(InvalidTransition) |
| LI-004 | Ready | Building | Err(InvalidTransition) |
| LI-005 | Stale | Failed | Err(InvalidTransition) |

### 2.4 StaleReason Variants

| ID | Test | Category | Expected |
|----|------|----------|----------|
| SR-001 | `SchemaVersionMismatch { expected, actual }` constructs | Happy path | Fields accessible |
| SR-002 | `SequenceGapDetected { gap_at }` constructs | Happy path | Fields accessible |
| SR-003 | `CorruptionDetected` constructs | Happy path | Variant matches |
| SR-004 | `ManualInvalidation` constructs | Happy path | Variant matches |

---

## 3. ProjectionResult & ProjectionRecord Tests

### 3.1 ProjectionResult

| ID | Test | Category | Expected |
|----|------|----------|----------|
| PR-001 | `ProjectionResult` fields match contract: state, events_applied, starting_sequence, ending_sequence, duration_ms, schema_version | Correctness | All fields present |
| PR-002 | `events_applied` equals actual event count | INV-001 | Exact count |
| PR-003 | `starting_sequence` and `ending_sequence` span correct range | Correctness | end >= start |
| PR-004 | `schema_version` matches projector's schema_version | INV-008 | Exact match |
| PR-005 | `duration_ms` is non-negative | Correctness | u64 >= 0 |

### 3.2 ProjectionRecord (Persisted State)

| ID | Test | Category | Expected |
|----|------|----------|----------|
| RC-001 | `ProjectionRecord` fields: projection_id, schema_version, state_bytes, sequence_range, checksum, created_at, updated_at | Correctness | All fields present |
| RC-002 | `checksum` computed from state_bytes is deterministic | INV-014 | Same bytes -> same checksum |
| RC-003 | `checksum` mismatch detected on load | INV-014 | Err(ChecksumMismatch) |
| RC-004 | `sequence_range` matches events used to build | INV-015 | (start, end) correct |
| RC-005 | `created_at` is immutable after first write | INV-016 | Subsequent writes don't change it |
| RC-006 | `updated_at` changes on each write | INV-016 | updated_at >= created_at |
| RC-007 | `state_bytes` round-trips through Serialize/Deserialize | Serde | Eq after round-trip |
| RC-008 | JSON round-trip preserves all fields | Serde | Eq after deserialize |

---

## 4. Determinism & Idempotency Invariant Tests (INV-001 through INV-004)

| ID | Invariant | Test Strategy |
|----|-----------|---------------|
| IV-001 | INV-001 (purity) | Call `project(state, event)` twice; assert byte-identical output |
| IV-002 | INV-002 (deterministic rebuild) | Rebuild projection from same event sequence; assert state is identical |
| IV-003 | INV-003 (no mutable state) | Create projector; call project(A); call project(B); verify B result is independent of A |
| IV-004 | INV-004 (initial_state) | Assert `initial_state()` returns zero-value equivalent for all field types |

---

## 5. Sequence Continuity Invariant Tests (INV-005 through INV-007)

| ID | Invariant | Test Strategy |
|----|-----------|---------------|
| IV-005 | INV-005 (sequence gap halts replay) | Provide events [1, 2, 4]; assert ReplayError::SequenceGap |
| IV-006 | INV-006 (strict ascending order) | Provide events [1, 3, 2]; assert error at sequence 2 |
| IV-007 | INV-007 (single instance_id) | Provide events from two instance_ids; assert ReplayError::InstanceMismatch |
| IV-008 | INV-005 (sequence gap triggers rebuild) | After SequenceGap, verify projection transitions to Stale and rebuild is initiated |

---

## 6. Version Compatibility Invariant Tests (INV-008 through INV-010)

| ID | Invariant | Test Strategy |
|----|-----------|---------------|
| IV-009 | INV-008 (compat window check before replay) | Set projection version outside window; assert rebuild triggered before replay starts |
| IV-010 | INV-009 (upcasting before replay) | Provide v0 events with v0→v1 upcaster; verify upcast happens before project() is called |
| IV-011 | INV-010 (StaleTooOld triggers rebuild) | Return ProjectionCompat::StaleTooOld; assert projection transitions to Stale immediately |

---

## 7. Self-Healing Protocol Tests (INV-011 through INV-013, ADR-037)

### 7.1 Staleness Detection

| ID | Test | Category | Expected |
|----|------|----------|----------|
| SH-001 | SchemaVersionMismatch detected at load time | INV-011 | Transition Ready → Stale |
| SH-002 | SequenceGapDetected detected during incremental update | INV-011 | Transition Ready → Stale |
| SH-003 | CorruptionDetected on checksum mismatch | INV-014 | Transition Ready → Stale |
| SH-004 | ManualInvalidation via explicit API | INV-011 | Transition Ready → Stale |
| SH-005 | Periodic health check detects drift | Protocol | StaleReason populated |

### 7.2 Rebuild Protocol

| ID | Test | Category | Expected |
|----|------|----------|----------|
| SH-006 | Stale → Rebuilding transition before serving | INV-012 | Rebuilding state before any reads |
| SH-007 | Full rebuild from sequence 1 produces correct state | Protocol | Matches fresh build |
| SH-008 | Full rebuild from post-snapshot produces correct state | Protocol | Matches fresh build from sequence 1 |
| SH-009 | Successful rebuild → Ready transition | Protocol | ProjectionCompleted event emitted |
| SH-010 | Failed rebuild → Failed transition | Protocol | ProjectionRebuildFailed event emitted |
| SH-011 | Failed state is terminal — no auto-retry | INV-013 | Err(TerminalStateTransition) on any subsequent transition |
| SH-012 | Manual reset from Failed → Building | Protocol | Reset succeeds, new build starts |
| SH-013 | Staleness detected mid-incremental triggers full rebuild | Protocol | Abort incremental, start full rebuild |
| SH-014 | Post-upcasting staleness check before replay | Protocol | StaleTooOld caught before project() called |

---

## 8. Storage Integrity Tests (INV-014 through INV-016)

| ID | Invariant | Test Strategy |
|----|-----------|---------------|
| IV-014 | INV-014 (checksum on load) | Write valid record; corrupt 1 byte of state_bytes; reload; assert ChecksumMismatch |
| IV-015 | INV-015 (sequence range match) | Build projection from events 5-20; verify record.sequence_range == (5, 20) |
| IV-016 | INV-016 (created_at immutable) | Write record at t=100; update at t=200; assert created_at == 100, updated_at == 200 |

---

## 9. Build Protocol Tests (Fresh Start)

| ID | Test | Category | Expected |
|----|------|----------|----------|
| BP-001 | Fresh build with empty event log | Happy path | initial_state(), 0 events_applied |
| BP-002 | Fresh build with single event | Happy path | state after 1 project() call |
| BP-003 | Fresh build with 100 events | Happy path | state after 100 project() calls, events_applied == 100 |
| BP-004 | Fresh build with sequence gap halts | Error path | Build fails, ProjectionResult not written |
| BP-005 | Fresh build with invalid event payload | Error path | Build fails with TransitionFailed or PayloadDecodeFailed |
| BP-006 | Fresh build writes ProjectionRecord to storage | Integration | Record persisted with correct fields |
| BP-007 | Fresh build emits ProjectionCompleted event | Observability | Event contains projection_id and events_applied |
| BP-008 | Fresh build emits ProjectionStarted event at beginning | Observability | Event contains projection_id and from_sequence |
| BP-009 | Fresh build emits ProjectionProgress events during replay | Observability | Events contain percent and at_sequence |

---

## 10. Incremental Update Protocol Tests (Catch-up)

| ID | Test | Category | Expected |
|----|------|----------|----------|
| IU-001 | Incremental update with 1 new event | Happy path | State updated, sequence_range extended |
| IU-002 | Incremental update with 0 new events | Edge case | No-op, state unchanged |
| IU-003 | Incremental update resumes from last_processed + 1 | Correctness | Starting sequence == record.sequence_range.1 + 1 |
| IU-004 | Incremental update validates checksum before proceeding | INV-014 | ChecksumMismatch triggers rebuild instead |
| IU-005 | Incremental update detects staleness mid-replay | Protocol | Abort, trigger full rebuild |
| IU-006 | Incremental update updates ProjectionRecord in storage | Integration | New state_bytes, extended sequence_range, updated checksum |
| IU-007 | Incremental update preserves created_at | INV-016 | created_at unchanged, updated_at changed |

---

## 11. Error Taxonomy Tests

### 11.1 ProjectionError (Top-Level)

| ID | Variant | Trigger |
|----|---------|---------|
| ET-001 | ProjectionError::Replay | Sequence gap during replay |
| ET-002 | ProjectionError::Version | Projection outside compat window |
| ET-003 | ProjectionError::Storage | Checksum mismatch on load |
| ET-004 | ProjectionError::Projector | Projector returns error |
| ET-005 | ProjectionError::State | Invalid state transition |

### 11.2 ReplayError

| ID | Variant | Trigger |
|----|---------|---------|
| ER-001 | ReplayError::InstanceMismatch | Events from different instance_ids |
| ER-002 | ReplayError::SequenceGap | Non-contiguous sequence numbers |
| ER-003 | ReplayError::SequenceDuplicate | Same sequence number twice |
| ER-004 | ReplayError::PayloadDecodeFailed | Malformed JSON payload |
| ER-005 | ReplayError::TransitionFailed | Invalid state machine transition |
| ER-006 | ReplayError::UnexpectedEventType | Unknown event payload type |
| ER-007 | ReplayError::UpcastingFailed | Upcaster chain failure |

### 11.3 ProjectionVersionError

| ID | Variant | Trigger |
|----|---------|---------|
| EV-001 | ProjectionVersionError::StaleProjection | Version outside window |
| EV-002 | ProjectionVersionError::MissingSchemaVersion | No version field in payload |
| EV-003 | ProjectionVersionError::InvalidSchemaVersionType | Version field is string/null |
| EV-004 | ProjectionVersionError::SchemaVersionExceedsMax | Version > max_supported |
| EV-005 | ProjectionVersionError::WindowMisconfigured | min >= max or min < 1 |
| EV-006 | ProjectionVersionError::UpcastingChainExhausted | No upcaster path from version A to B |
| EV-007 | ProjectionVersionError::NoUpcasterRegistered | Version has no registered upcaster |

### 11.4 ProjectionStateError

| ID | Variant | Trigger |
|----|---------|---------|
| ES-001 | ProjectionStateError::InvalidTransition | Ready → Building attempted |
| ES-002 | ProjectionStateError::TerminalStateTransition | Failed → Ready attempted |
| ES-003 | ProjectionStateError::RebuildNotStale | Ready → Rebuilding attempted |
| ES-004 | ProjectionStateError::StateCorrupted | Hash mismatch on loaded state |

### 11.5 Existing ProjectionError (vo-storage compat)

| ID | Variant | Trigger |
|----|---------|---------|
| EC-001 | ProjectionError::StaleProjection(u8, u8, u8) | Version 1 with window [3,7] |
| EC-002 | ProjectionError::MissingSchemaVersion | JSON without "version" field |
| EC-003 | ProjectionError::InvalidSchemaVersionType | version: "abc" |
| EC-004 | ProjectionError::SchemaVersionExceedsMax | version: 100, max: 5 |
| EC-005 | ProjectionError::WindowMisconfigured | min: 0, max: 5 |
| EC-006 | ProjectionError::BatchDecodeFailed | Invalid JSON in batch |

---

## 12. Upcaster Integration Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| UC-001 | Events at current version pass through without upcasting | Happy path | project() sees original event |
| UC-002 | Events at older version are upcast before project() | INV-009 | project() sees upcasted event |
| UC-003 | Mixed-version events all upcast to current before replay | INV-009 | All events at max version before project() |
| UC-004 | Upcaster chain failure halts replay | Error path | ReplayError::UpcastingFailed |
| UC-005 | No upcaster registered for version returns error | Error path | ProjectionVersionError::NoUpcasterRegistered |
| UC-006 | Upcasting chain exhaustion returns error | Error path | ProjectionVersionError::UpcastingChainExhausted |
| UC-007 | Upcasted events produce same state as native-version events | INV-002 | Deterministic equivalence |

---

## 13. Observability Event Tests (ProjectionEvent)

| ID | Event | Trigger |
|----|-------|---------|
| OE-001 | ProjectionStarted { projection_id, from_sequence } | Build initiated |
| OE-002 | ProjectionProgress { projection_id, percent, at_sequence } | During replay (at intervals) |
| OE-003 | ProjectionCompleted { projection_id, events_applied } | Build/rebuild finished |
| OE-004 | ProjectionStale { projection_id, reason } | Staleness detected |
| OE-005 | ProjectionRebuildStarted { projection_id, reason } | Rebuild initiated |
| OE-006 | ProjectionRebuildFailed { projection_id, error } | Rebuild failed |

---

## 14. Property-Based Tests (proptest)

| ID | Property | Strategy |
|----|----------|----------|
| PP-001 | **Projector purity** (INV-001) | Arbitrary (state, event) pairs; project() called twice; assert equal |
| PP-002 | **Deterministic rebuild** (INV-002) | Arbitrary event sequence; rebuild twice; assert byte-identical state |
| PP-003 | **Sequence ordering** (INV-006) | Arbitrary ordered events replayed; compare against shuffled; only ordered succeeds |
| PP-004 | **Compat window partition exhaustiveness** | Arbitrary (version, window_min, window_max); exactly one ProjectionCompat variant |
| PP-005 | **Checksum determinism** (INV-014) | Arbitrary state_bytes; compute checksum twice; assert equal |
| PP-006 | **created_at immutability** (INV-016) | Build record; update N times; created_at never changes |
| PP-007 | **State machine transition validity** | Arbitrary (current_state, event) pairs; transition either succeeds or returns valid error variant |
| PP-008 | **Initial state is zero-value** (INV-004) | Arbitrary projector impl; initial_state() matches Default::default() |
| PP-009 | **Upcast then replay equals native replay** (INV-002) | Arbitrary events at version N; upcast to max; replay; compare with native max-version replay |
| PP-010 | **ProjectionResult field consistency** | Arbitrary event sequence; events_applied == len(events) minus terminal-stop count |
| PP-011 | **Sequence range matches events** (INV-015) | Arbitrary events; record.sequence_range == (events.first().sequence, events.last().sequence) |
| PP-012 | **Progress monotonicity** | During build, percent values are non-decreasing |

---

## 15. Integration Tests (Cross-Crate Protocol)

| ID | Test | Category | Expected |
|----|------|----------|----------|
| IT-001 | Build projection end-to-end: create projector, replay events, persist record | Integration | Valid ProjectionRecord in storage |
| IT-002 | Incremental update: load record, replay new events, persist updated record | Integration | Updated record with extended range |
| IT-003 | Self-healing cycle: detect stale → rebuild → verify ready | Integration | Projection back in Ready state |
| IT-004 | Self-healing with failure: detect stale → rebuild fails → Failed state | Integration | Projection in Failed state, no auto-retry |
| IT-005 | Upcaster + replay integration: v0 events upcast, replayed, persisted | Integration | Correct state at schema_version == max |
| IT-006 | Compat check + replay: check_projection_compat before replay, reject stale | Integration | Rebuild triggered, not replay |
| IT-007 | Checksum verification on load: corrupt stored bytes, detect on load | Integration | Rebuild triggered |
| IT-008 | Full lifecycle: build → serve → detect stale → rebuild → serve | Integration | Continuous availability (except during rebuild) |

---

## 16. Edge Case Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| ED-001 | Empty event log produces initial_state | Edge | events_applied == 0 |
| ED-002 | Single event replay | Edge | events_applied == 1 |
| ED-003 | Projection with u8::MAX schema version | Boundary | Compat check handles overflow |
| ED-004 | Compatibility window at minimum (min=1, max=1) | Boundary | Only version 1 is Fresh |
| ED-005 | Sequence at u64::MAX | Boundary | No overflow in sequence arithmetic |
| ED-006 | Zero-length state_bytes after serialization | Edge | Checksum of empty bytes valid |
| ED-007 | Rebuild from empty snapshot (no prior state) | Edge | Behaves like fresh build |
| ED-008 | Multiple staleness reasons in sequence | Edge | First detection wins, subsequent ignored |
| ED-009 | Projector that handles unknown event types gracefully | Edge | Logged but replay continues |
| ED-010 | Progress reporting at 0% and 100% boundaries | Boundary | Events emitted correctly |
| ED-011 | Concurrent staleness detection and rebuild initiation | Edge | Rebuild not double-initiated |
| ED-012 | Large event sequence (10,000 events) rebuild performance | Performance | Completes in reasonable time |

---

## 17. Serde & Serialization Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| SE-001 | ProjectionState JSON round-trip | Serde | Eq after deserialize |
| SE-002 | StaleReason JSON round-trip | Serde | Eq after deserialize |
| SE-003 | ProjectionResult JSON round-trip | Serde | Eq after deserialize |
| SE-004 | ProjectionRecord JSON round-trip | Serde | Eq after deserialize |
| SE-005 | ProjectionEvent JSON round-trip | Serde | Eq after deserialize |
| SE-006 | ProjectionStateError JSON round-trip | Serde | Eq after deserialize |
| SE-007 | ProjectionVersionError JSON round-trip | Serde | Eq after deserialize |
| SE-008 | State bytes serialization is deterministic | Correctness | Same state -> same bytes |

---

## 18. Error Display & Debug Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| DD-001 | ProjectionError::Display contains variant info | Correctness | Human-readable string |
| DD-002 | ProjectionError::Debug contains fields | Correctness | Struct-like debug output |
| DD-003 | ReplayError::Display contains sequence info | Correctness | Sequence number in message |
| DD-004 | ProjectionStateError::Display contains state names | Correctness | From/to states in message |
| DD-005 | ProjectionVersionError::Display contains version numbers | Correctness | Version values in message |
| DD-006 | All error types implement std::error::Error | Trait bound | `.source()` chain works |

---

## Test File Organization

```
crates/vo-core/src/replay/
  mod.rs                              # Module root
  engine.rs                           # ReplayEngine (existing)
  types.rs                            # ReplayResult, ReplayError (existing)
  test_helpers.rs                     # Shared test helpers (existing)
  tests.rs                            # Happy-path tests (existing)
  error_tests.rs                      # Error-path tests (existing)
  upcaster_tests.rs                   # Upcaster integration tests (existing)
  projection/
    mod.rs                            # Projection module root
    projector_tests.rs                # PT-*, PE-* (Projector trait)
    state_machine_tests.rs            # LS-*, LT-*, LI-*, SR-* (ProjectionState lifecycle)
    result_tests.rs                   # PR-* (ProjectionResult)
    record_tests.rs                   # RC-* (ProjectionRecord)
    build_protocol_tests.rs           # BP-* (Build protocol)
    incremental_tests.rs              # IU-* (Incremental update)
    self_healing_tests.rs             # SH-* (Self-healing protocol)
    observability_tests.rs            # OE-* (ProjectionEvent)
    invariant_tests.rs                # IV-* (INV-001 through INV-016)
    proptest.rs                       # PP-* (Property-based tests)
    integration_tests.rs              # IT-* (Cross-crate integration)
    edge_case_tests.rs                # ED-* (Edge cases)
    serde_tests.rs                    # SE-* (Serialization)
    error_taxonomy_tests.rs           # ET-*, ER-*, EV-*, ES-* (Error variants)
    display_tests.rs                  # DD-* (Error display/debug)

crates/vo-storage/src/projection_compat/
  types.rs                            # ProjectionCompat, ProjectionError (existing)
  calc.rs                             # Pure calc functions (existing)
  actions.rs                          # Fallible actions (existing)
  tests.rs                            # Existing compat tests (EC-*)
```

## Test Count Summary

| Category | Count |
|----------|-------|
| Projector trait interface | 10 |
| ProjectionState lifecycle | 18 |
| ProjectionResult & ProjectionRecord | 13 |
| Determinism & idempotency invariants | 4 |
| Sequence continuity invariants | 4 |
| Version compatibility invariants | 3 |
| Self-healing protocol | 14 |
| Storage integrity invariants | 3 |
| Build protocol | 9 |
| Incremental update protocol | 7 |
| Error taxonomy (top-level + sub-enums) | 27 |
| Upcaster integration | 7 |
| Observability events | 6 |
| Property-based tests | 12 |
| Integration tests (cross-crate) | 8 |
| Edge cases | 12 |
| Serde & serialization | 8 |
| Error display & debug | 6 |
| **Total** | **181** |
