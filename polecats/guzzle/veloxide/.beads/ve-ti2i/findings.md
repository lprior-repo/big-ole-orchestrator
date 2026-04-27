# ADR-016 Review Findings: Atomic Storage Snapshots

**Bead**: ve-ti2i
**Reviewer**: polecat/guzzle
**Date**: 2026-04-24
**Status**: QA/Audit — No Code Changes

---

## 1. ADR Requirements Summary

ADR-016 mandates two core guarantees:

1. **Atomic WriteBatches**: `DbWriterActor` uses `fjall::Batch` for every control-plane transition. A transition must atomically update ALL touched partitions: `events`, `instances`, `timers`, `dedupe`, `effects`, `leases`, and `snapshots`.

2. **Periodic State Snapshotting**: Every N events, actor serializes state and sends `SnapshotTaken { sequence_number, state_bytes }` to `DbWriterActor`. The snapshot is written as part of the SAME atomic batch as the triggering event.

---

## 2. Code That EXISTS

### 2.1 `SnapshotData` Struct
**File**: `vo-core/src/db_writer_message.rs:60-102`

```rust
pub struct SnapshotData {
    sequence_number: SequenceNumber,
    schema_version: u16,
    state_bytes: Vec<u8>,
}
```
- Invariant: `state_bytes` must be non-empty (enforced via `Option<Self>` constructor returning `None` for empty)
- Schema version tracked for upcasting chain (ADR-035)
- Accessors: `sequence_number()`, `schema_version()`, `state_bytes()`

### 2.2 `SnapshotHeader` Struct
**File**: `vo-storage/src/snapshots/mod.rs:10-27`

```rust
pub struct SnapshotHeader {
    pub version: u16,          // CURRENT_SNAPSHOT_VERSION = 1
    pub sequence_number: u64,
    pub instance_id: InstanceId,
    pub checksum: u32,          // crc32fast::hash of state JSON
}
```
- Format: `header_json | state_json` (pipe delimiter at line 94)
- Checksum uses `crc32fast::hash` for integrity verification

### 2.3 `AtomicSnapshotWriter`
**File**: `vo-storage/src/snapshots/mod.rs:52-117`

```rust
pub struct AtomicSnapshotWriter<'a> {
    db: &'a fjall::Database,
    snapshot_partition: Keyspace,
}

impl AtomicSnapshotWriter {
    pub fn new(db: &'a fjall::Database) -> Result<Self, StorageError>
    pub fn write_snapshot(&self, batch: &mut fjall::OwnedWriteBatch,
        instance_id: InstanceId, sequence: u64, state: &InstanceState) -> Result<(), StorageError>
    pub fn write_snapshot_atomic(&self, instance_id, sequence, state) -> Result<(), StorageError>
}
```

`write_snapshot()` adds to a provided batch; `write_snapshot_atomic()` creates its own batch and commits. Both compute checksum before writing.

### 2.4 `SnapshotPolicy`
**File**: `vo-storage/src/snapshots/mod.rs:30-50`

```rust
pub enum SnapshotPolicy {
    EveryNEvents(u64),  // Default: 100
    Disabled,
}
```

`snapshot_policy.should_snapshot(current_sequence)` returns true when `current_sequence.is_multiple_of(n)`.

### 2.5 `DbWriterMessage` Enum
**File**: `vo-core/src/db_writer_message.rs:113-163`

```rust
pub enum DbWriterMessage {
    TakeSnapshot { instance_id, sequence_number, snapshot_data },
    AtomicTransition {
        step_id: Option<StepId>,
        instance_status: Option<InstanceStatus>,
        timer_ops: Vec<TimerOp>,
        snapshot: Option<SnapshotData>,
        event: EventEnvelope,
    },
    // ... other variants
}
```

The `AtomicTransition` message type contains all components that must be atomically committed, including optional snapshot.

### 2.6 `SnapshotRecovery`
**File**: `vo-storage/src/snapshot_recovery.rs:330-390`

Throttled crash recovery coordinator with:
- Token-bucket throttle (`ThrottleState`) limiting concurrent recoveries
- `select_best_recovery_point()` using `snapshot_load_latest()`
- `try_acquire_recovery_slot()` / `release_recovery_slot()`
- `RecoveryPoint` with `events_to_replay(current_sequence)` for bounded rehydration

**Kani proofs** exist verifying throttle invariants (bounded token count, active recoveries ≤ max).

### 2.7 `snapshot_load_latest_with_compat`
**File**: `vo-storage/src/snapshots/mod.rs:332-`

Schema version compatibility checking:
- Discards snapshots below `min_version` or above `engine_version`
- Discards legacy format (no header, version 0)
- Returns `CompatSnapshotLoad::Loaded` or `CompatSnapshotLoad::Discarded`

### 2.8 `compact_snapshots`
**File**: `vo-storage/src/snapshots/mod.rs:180-199`

Compacts to keep last N snapshots for an instance, sorted by sequence descending, deleting older ones.

### 2.9 Partition Layout
**File**: `vo-storage/src/partitions.rs`

| Partition | Class | Notes |
|-----------|-------|-------|
| `events` | Hot | Key: `<instance_id><sequence>` |
| `instances` | Hot | Key: `<status><created_at><instance_id>` |
| `timers` | Hot | Key: `<fire_at_ms><instance_id><timer_id>` |
| `snapshots` | Cold | Key: `<instance_id><sequence>`, bloom_filter=0 |
| `dedupe` | Hot | Exactly-once ingress deduplication |
| `effects` | Hot | EffectPrepared/EffectCommitted journal |
| `leases` | Hot | Monotonic fence tokens |
| `receipts` | Hot | Execution receipts |

---

## 3. CRITICAL GAP: `DbWriterActor` Not Found

### 3.1 The Problem

ADR-016 explicitly requires:
> "The `DbWriterActor` is mandated to use `fjall::Batch` for every single control-plane transition."

However, **the `DbWriterActor` implementation does not exist in the codebase**:

- `find /home/lewis/src/veloxide/crates -name "db_writer_actor.rs"` returns **NO RESULTS**
- `find /home/lewis/src/veloxide/crates -name "atomic_batch.rs"` returns **NO RESULTS**
- `StorageEngine` (partitions.rs:283-322) only has `dedupe_store`, `effect_journal`, `lease_store` — **NO snapshots field, NO atomic batch coordinator**

### 3.2 Consequence

Without `DbWriterActor`:
1. **No verification possible** that `AtomicTransition` event+snapshot are committed in the SAME `fjall::Batch`
2. **No single place** coordinates atomic multi-partition updates (events+instances+timers+snapshots)
3. The `DbWriterMessage::AtomicTransition` exists as a message type but **no actor processes it**
4. The `AtomicSnapshotWriter` can add snapshots to a batch, but there's no code that also adds the event to the same batch

### 3.3 Related TODO

**File**: `vo-storage/src/partitions.rs:308`
```rust
// TODO: event_store module removed during fjall 3 migration - needs reimplementation
```

This confirms that the event store (which should write to the `events` partition) was removed and has not been reimplemented.

---

## 4. Snapshot Storage — Partial Compliance

The snapshot storage layer (`vo-storage/src/snapshots/`) is reasonably complete:

| Feature | Status |
|---------|--------|
| `SnapshotHeader` with checksum | ✅ Implemented |
| `SnapshotPolicy` (every N events) | ✅ Implemented |
| `AtomicSnapshotWriter::write_snapshot()` | ✅ Implemented |
| `AtomicSnapshotWriter::write_snapshot_atomic()` | ✅ Implemented |
| `snapshot_load_latest()` | ✅ Implemented |
| `snapshot_load_latest_with_compat()` | ✅ Implemented |
| `compact_snapshots()` | ✅ Implemented |
| `SnapshotRecovery` with throttle | ✅ Implemented |
| `SnapshotData` (message type) | ✅ Implemented |
| Kani proofs for throttle | ✅ Implemented |

**However**, all of these are **storage primitives**. The critical ADR requirement — that snapshots be written in the SAME atomic batch as the triggering event — **cannot be verified without `DbWriterActor`**.

---

## 5. Concurrent Snapshot + Mutation — NOT TESTED

ADR-016 requires: "Test concurrent snapshot + mutation."

No concurrent tests were found for:
- Simultaneous snapshot writes and event appends to the same instance
- Concurrent `AtomicSnapshotWriter::write_snapshot()` calls on the same batch
- Concurrent snapshot compaction while snapshots are being written

---

## 6. Partition Compliance

| ADR-016 Requirement | Implementation |
|---------------------|----------------|
| `events` partition | ⚠️ Removed (fjall 3 migration TODO) |
| `instances` partition | ✅ Partition defined, no direct store |
| `timers` partition | ✅ Partition defined, no direct store |
| `dedupe` partition | ✅ `FjallDedupeStore` exists |
| `effects` partition | ✅ `FjallEffectJournal` exists |
| `leases` partition | ✅ `FjallLeaseStore` exists |
| `snapshots` partition | ✅ Storage primitives exist, writer missing |

---

## 7. Summary Verdict

| ADR-016 Requirement | Verdict |
|---------------------|---------|
| Atomic WriteBatches via `DbWriterActor` | ❌ **MISSING** — actor not found |
| All partitions atomically updated | ❌ **UNVERIFIABLE** — no coordinator |
| Periodic snapshotting (every N events) | ⚠️ **PARTIAL** — policy exists, trigger missing |
| Snapshot in same batch as triggering event | ❌ **UNVERIFIABLE** — no `DbWriterActor` |
| Snapshot checksum verification | ✅ Implemented |
| Snapshot schema version compatibility | ✅ Implemented |
| Rehydration from snapshot (bounded O(N)) | ⚠️ **PARTIAL** — `SnapshotRecovery` exists, but event replay path unclear |
| Crash recovery throttle | ✅ Implemented with Kani proofs |

### Overall: **NON-COMPLIANT** with ADR-016

The snapshot storage primitives are in place, but the atomic write batch coordinator (`DbWriterActor`) that ADR-016 mandates is **absent from the codebase**. The `AtomicTransition` message type exists but has no corresponding actor to process it. Additionally, the `event_store` module was removed during a fjall 3 migration and has not been reimplemented.

---

## 8. Recommendations

1. **Implement `DbWriterActor`** — The core actor that receives `DbWriterMessage::AtomicTransition` and commits all partitions in a single `fjall::OwnedWriteBatch`

2. **Re-implement event_store** — The `events` partition write path is needed for atomicity

3. **Add concurrent snapshot + mutation tests** — Test that simultaneous snapshot writes and event appends maintain consistency

4. **Wire snapshots into `StorageEngine`** — Currently `StorageEngine` doesn't manage snapshots; need to add snapshot partition handling

5. **Document the fjall 3 migration** — The removal of `event_store` and `atomic_batch.rs` suggests a refactoring; the current state should be explicitly documented

---

## 9. Files Reviewed

- `vo-core/src/db_writer_message.rs` — `SnapshotData`, `DbWriterMessage` enum
- `vo-storage/src/snapshots/mod.rs` — `SnapshotHeader`, `AtomicSnapshotWriter`, `SnapshotPolicy`, load/compact functions
- `vo-storage/src/snapshot_recovery.rs` — `SnapshotRecovery`, throttle, `RecoveryPoint`
- `vo-storage/src/partitions.rs` — `StorageEngine`, partition definitions, HOT/COLD split
- `vo-core/src/snapshot_compat.rs` — (referenced, exists)
- ADR-016 document from overlord worktree
