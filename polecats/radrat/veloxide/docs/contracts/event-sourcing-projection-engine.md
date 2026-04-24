# Contract: Event Sourcing Projection Engine

## 1. Purpose

Defines the contract for the veloxide event sourcing projection engine — the system that transforms immutable event sequences into materialized read models (projections). This contract establishes the authoritative types, invariants, and error taxonomy for all projection operations.

The projection engine is the bridge between the canonical event log and operator-facing views. Per ADR-037, all operator-facing projections are **rebuildable** from canonical sources, enabling self-healing when schema drift or corruption occurs.

## 2. Source ADRs

- `docs/adr/v2/ADR-037-v2-rebuildable-projections-and-self-healing.md` (projection rebuildability)
- `docs/adr/v2/ADR-027-v2-deterministic-event-sourced-replay.md` (replay semantics)
- `docs/adr/v2/ADR-035-v2-event-schema-evolution-and-upcasting.md` (upcasting)
- `docs/adr/v2/ADR-016-v2-atomic-storage-snapshots.md` (snapshot integration)

## 3. Projection Engine Types

### 3.1 ProjectionEngine

The main projection engine — a stateless, deterministic processor that transforms event sequences into projection states.

```
ProjectionEngine {
  upcaster_registry: UpcasterRegistry,
  compatibility_window: ProjectionCompatibilityWindow,
}
```

**Responsibilities:**
- Coordinate event replay through projector functions
- Validate schema version compatibility before replay
- Detect stale projections and trigger rebuild workflows
- Ensure deterministic, idempotent replay semantics

### 3.2 Projector Trait

The interface for specific projection implementations. Each projection type implements this trait.

```
trait Projector<S, E> {
  type ProjectionState: Clone + Default + serde::Serialize;
  type Error: Into<ProjectionError>;

  fn project(&self, state: Self::ProjectionState, event: &E) -> Result<Self::ProjectionState, Self::Error>;

  fn initial_state() -> Self::ProjectionState;

  fn schema_version(&self) -> u8;
}
```

**Type Parameters:**
- `S` — The projection's state type (must be `Clone + Default + Serialize`)
- `E` — The event type this projector consumes (typically `EventEnvelope` or `EventPayload`)

**Methods:**
- `project(state, event)` — Pure function that transforms state given an event
- `initial_state()` — Returns the starting state for a new projection
- `schema_version()` — Returns the current schema version for this projection

### 3.3 ProjectionState

The lifecycle state machine for a projection's build/rebuild cycle.

```
ProjectionState {
  Building,           // Initial state: projection being built from event log
  Ready,               // Projection is current and usable
  Stale {             // Projection is stale but usable
    detected_at: u64,  // Timestamp when staleness was detected
    reason: StaleReason,
  },
  Rebuilding {        // Projection is being rebuilt
    progress: u32,     // Percentage complete (0-100)
    from_sequence: u64,
  },
  Failed {            // Terminal: rebuild failed
    reason: String,
    attempted_at: u64,
  },
}
```

### 3.4 StaleReason

Why a projection became stale.

```
StaleReason {
  SchemaVersionMismatch { expected: u8, actual: u8 },
  SequenceGapDetected { gap_at: u64 },
  CorruptionDetected,
  ManualInvalidation,
}
```

### 3.5 ProjectionResult

Result of a complete projection replay.

```
ProjectionResult<S> {
  state: S,                    // Final projection state
  events_applied: u64,         // Count of events processed
  starting_sequence: u64,       // First sequence processed
  ending_sequence: u64,        // Last sequence processed
  duration_ms: u64,            // Replay duration
  schema_version: u8,          // Schema version of final state
}
```

### 3.6 ProjectionRecord

Persisted projection state in storage.

```
ProjectionRecord {
  projection_id: String,
  schema_version: u8,
  state_bytes: Vec<u8>,
  sequence_range: (u64, u64),   // (start, end) inclusive
  checksum: u64,
  created_at: u64,
  updated_at: u64,
}
```

### 3.7 ProjectionEvent

Internal events emitted by the projection engine for observability.

```
ProjectionEvent {
  ProjectionStarted { projection_id: String, from_sequence: u64 },
  ProjectionProgress { projection_id: String, percent: u32, at_sequence: u64 },
  ProjectionCompleted { projection_id: String, events_applied: u64 },
  ProjectionStale { projection_id: String, reason: StaleReason },
  ProjectionRebuildStarted { projection_id: String, reason: StaleReason },
  ProjectionRebuildFailed { projection_id: String, error: String },
}
```

## 4. Invariants (INV-*)

### Determinism & Idempotency

- **INV-001**: `project(state, event)` must be a pure function — same (state, event) always produces identical new state
- **INV-002**: Rebuilding a projection from the same event sequence must produce byte-for-byte identical state
- **INV-003**: Projector implementations must not retain mutable state between calls
- **INV-004**: `initial_state()` must produce a semantically empty state (zero values for all fields)

### Sequence Continuity

- **INV-005**: Event replay requires continuous sequences — any `SequenceGap` must halt replay and trigger rebuild
- **INV-006**: Replay processes events in strict ascending `sequence` order for a given `instance_id`
- **INV-007**: All events for a replay must share the same `instance_id` — mixed-instance replay is a fatal error

### Version Compatibility

- **INV-008**: Projection schema version must fall within `ProjectionCompatibilityWindow` before replay begins
- **INV-009**: Upcasting happens **before** replay — the replay engine never sees pre-upcast events
- **INV-010**: A projection marked `StaleTooOld` cannot be used and must trigger automatic rebuild

### Self-Healing (ADR-037)

- **INV-011**: Detection of `ProjectionCompat::StaleTooOld` must immediately transition projection to `Stale` state
- **INV-012**: Transition from `Stale` to `Rebuilding` must occur before serving requests from the stale projection
- **INV-013**: `Failed` state is terminal — manual intervention required to reset or delete

### Storage Integrity

- **INV-014**: `ProjectionRecord.checksum` must be verified on load — mismatch triggers rebuild
- **INV-015**: Sequence range in `ProjectionRecord` must match actual events used to build it
- **INV-016**: `created_at` must be immutable after first write — updates only affect `updated_at`

## 5. Error Taxonomy

### 5.1 Top-Level ProjectionError

```rust
enum ProjectionError {
  // Replay errors
  Replay(ReplayError),

  // Version compatibility errors
  Version(ProjectionVersionError),

  // Storage errors
  Storage(StorageError),

  // Projector-specific errors
  Projector(String),

  // State machine errors
  State(ProjectionStateError),
}
```

### 5.2 ReplayError

Errors from the underlying replay engine (vo-core).

```rust
enum ReplayError {
  InstanceMismatch { expected: String, actual: String },
  SequenceGap { expected: u64, actual: u64, at_index: usize },
  SequenceDuplicate { sequence: u64, first_at_index: usize, second_at_index: usize },
  PayloadDecodeFailed { sequence: u64, source: String },
  TransitionFailed { sequence: u64, state: String, reason: String },
  UnexpectedEventType { payload_type: String, sequence: u64 },
  UpcastingFailed { sequence: u64, reason: String },
}
```

### 5.3 ProjectionVersionError

Schema version and compatibility errors.

```rust
enum ProjectionVersionError {
  // Projection schema version outside compatibility window
  StaleProjection { version: u8, window_min: u8, window_max: u8 },

  // No schema version field found
  MissingSchemaVersion,

  // Schema version field not valid u8
  InvalidSchemaVersionType,

  // Projection version exceeds engine max
  SchemaVersionExceedsMax { version: u8, max: u8 },

  // Compatibility window misconfigured
  WindowMisconfigured { min: u8, max: u8 },

  // Upcasting chain exhausted or circular
  UpcastingChainExhausted { from_version: u8, target_version: u8 },

  // No upcaster registered for version
  NoUpcasterRegistered { version: u8 },
}
```

### 5.4 ProjectionStateError

State machine transition errors.

```rust
enum ProjectionStateError {
  // Invalid state transition attempted
  InvalidTransition { from: ProjectionState, event: ProjectionEventType },

  // Transition from terminal state not allowed
  TerminalStateTransition { state: ProjectionState, attempted: ProjectionEventType },

  // Rebuild attempted on non-stale projection
  RebuildNotStale { state: ProjectionState },

  // State corruption detected
  StateCorrupted { expected_hash: String, actual_hash: String },
}
```

### 5.5 StorageError

Storage-layer errors (from vo-storage).

```rust
enum StorageError {
  CorruptKey,
  SequenceGap,
  CorruptEventPayload,
  UnsupportedVersion,
  SerializationFailed,
  DeserializationFailed,
  FjallError,
  InvalidKey,
  ChecksumMismatch,
  // ... other variants from vo-storage
}
```

## 6. Projection Engine Protocol

### 6.1 Build Projection (Fresh Start)

```
1. Validate compatibility window is configured
2. Create Projector<S, E> instance
3. Call initial_state() to get starting state
4. Iterate events via EventReplayIterator
5. For each event:
   a. Upcast to current schema version (via UpcasterRegistry)
   b. Call projector.project(state, event)
   c. Update progress counter
   d. Check for sequence gaps — if found, halt and trigger rebuild
6. Serialize final state to bytes
7. Compute checksum
8. Write ProjectionRecord to storage
9. Emit ProjectionCompleted event
```

### 6.2 Incremental Update (Catch-up)

```
1. Load existing ProjectionRecord
2. Validate checksum
3. Determine starting sequence (last_processed + 1)
4. Resume replay from that point
5. If staleness detected mid-replay, abort and trigger full rebuild
6. Update ProjectionRecord with new state and sequence range
```

### 6.3 Self-Healing Rebuild (ADR-037)

```
1. Detect staleness via ProjectionCompat check
2. Transition state: Ready → Stale
3. Emit ProjectionStale event
4. Transition state: Stale → Rebuilding
5. Emit ProjectionRebuildStarted event
6. Perform full rebuild from sequence 1 (or post-snapshot)
7. On success: Transition to Ready, emit ProjectionCompleted
8. On failure: Transition to Failed, emit ProjectionRebuildFailed
```

### 6.4 Staleness Detection Points

Staleness is checked at:
- **Load time**: Before serving any projection request
- **Replay start**: Before beginning incremental update
- **Periodic health checks**: Background scan for drift
- **Post-upcasting**: After upcasting, before replay

## 7. Constraints

- **Pure project()**: The `project` function must have no side effects, no I/O, and no dependence on external state
- **No mutable projector state**: `Projector` implementations must be stateless between `project()` calls
- **Exhaustive variant handling**: Projectors must handle all event variants — unknown variants should be logged but not halt replay
- **Deterministic only**: The engine rejects non-deterministic inputs (wall-clock time, random values) in projector code
- **Rebuildable only**: No projection is considered authoritative — all can be rebuilt from event log
- **Checksummed storage**: All persisted projections must carry a checksum for integrity verification

## 8. Relevant Files

- `crates/vo-core/src/replay/engine.rs` (existing replay engine)
- `crates/vo-core/src/replay/types.rs` (existing ReplayResult, ReplayError)
- `crates/vo-types/src/events/envelope.rs` (EventEnvelope)
- `crates/vo-types/src/events/payload.rs` (EventPayload)
- `crates/vo-storage/src/projection_compat/types.rs` (existing ProjectionCompat, ProjectionError)
- `crates/vo-storage/src/projection_compat/actions.rs` (validation actions)
- `crates/vo-storage/src/query/mod.rs` (EventReplayIterator)
- `crates/vo-core/src/upcaster/registry.rs` (UpcasterRegistry)

## 9. Acceptance Criteria

- [ ] `Projector` trait captures the complete interface for projection implementations
- [ ] `ProjectionState` state machine covers all valid states with no invalid transitions
- [ ] `ProjectionError` taxonomy is exhaustive and covers all failure modes
- [ ] All invariants (INV-001 through INV-016) are formally stated and testable
- [ ] Self-healing protocol aligns with ADR-037 rebuildability requirements
- [ ] Upcaster integration is specified as a pre-replay step
- [ ] Checksummed storage provides integrity verification
- [ ] Protocol sections (build, update, rebuild) provide implementation guidance
- [ ] Contract references existing ADR documentation for foundational decisions
- [ ] No new dependencies introduced beyond existing crate boundaries
