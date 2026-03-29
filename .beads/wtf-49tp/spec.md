# Bead Specification: vo-49tp

```yaml
id: vo-49tp
title: "instance: Implement snapshot trigger"
status: ready
priority: 2
type: feature
effort_estimate: "1hr"
assigned: null
created: "2026-03-23"
---
```

## Section 1: Objective

Replace the stub at `crates/vo-actor/src/instance/handlers.rs:215-222` (`handle_snapshot_trigger`) with a real implementation that serializes the current `ParadigmState`, writes a `SnapshotRecord` to sled, publishes a `SnapshotTaken` JetStream event, and resets `events_since_snapshot`. This completes ADR-019 snapshotting for live event processing.

## Section 2: Scope

**In scope:**
- Replace `handle_snapshot_trigger` body in `handlers.rs`
- Make the function `async` (caller `inject_event` already awaits)
- Serialize `state.paradigm_state` to msgpack via `rmp_serde`
- Call `crate::snapshot::write_instance_snapshot` with event_store, snapshot_db, namespace, instance_id, last_applied_seq, state_bytes
- Reset `state.events_since_snapshot = 0` on success
- Log errors on failure (non-fatal — system falls back to full replay)

**Out of scope:**
- Recovery path changes (already handled in `init.rs:19-26`)
- `SnapshotTaken` event handling in event apply
- Integration/NATS tests

## Section 3: Affected Files

| File | Action | Lines |
|------|--------|-------|
| `crates/vo-actor/src/instance/handlers.rs` | MODIFY | 215-222 (replace stub) |
| `crates/vo-storage/src/snapshots.rs` | READ-ONLY | `write_snapshot`, `SnapshotRecord::new` |

No changes to `vo-storage`. The `snapshot.rs` module in `vo-actor` already contains `write_instance_snapshot` which is the entry point.

## Section 4: Dependencies

**Internal (already resolved):**
- `crate::snapshot::write_instance_snapshot` — exists at `crates/vo-actor/src/snapshot.rs:47`, signature:
  ```rust
  pub async fn write_instance_snapshot(
      event_store: &dyn EventStore,
      db: &sled::Db,
      namespace: &NamespaceId,
      instance_id: &InstanceId,
      last_applied_seq: u64,
      state_bytes: Bytes,
  ) -> Result<SnapshotResult, VoError>
  ```
- `rmp_serde::to_vec_named` — available in workspace Cargo.toml

**External:**
- `rmp_serde` — msgpack serialization (already in `crates/vo-actor/Cargo.toml`)
- `sled` — embedded KV (already in `crates/vo-actor/Cargo.toml`)
- `bytes::Bytes` — already imported in `handlers.rs:6`

## Section 5: Current State

The stub at `handlers.rs:215-222`:
```rust
fn handle_snapshot_trigger(state: &mut InstanceState) {
    tracing::debug!(
        instance_id = %state.args.instance_id,
        total = state.total_events_applied,
        "snapshot trigger (stub — see vo-flbh)"
    );
    state.events_since_snapshot = 0;
}
```

Problems:
1. Synchronous — needs to be `async` to call `write_instance_snapshot`
2. Does not serialize `paradigm_state`
3. Does not write to sled
4. Does not publish `SnapshotTaken` to JetStream
5. Resets counter even on failure

## Section 6: Target State

After implementation:
```rust
async fn handle_snapshot_trigger(state: &mut InstanceState) -> Result<(), ActorProcessingErr> {
    let event_store = state.args.event_store.as_ref()
        .ok_or_else(|| ActorProcessingErr::from("snapshot requires event_store"))?;
    let db = state.args.snapshot_db.as_ref()
        .ok_or_else(|| ActorProcessingErr::from("snapshot requires snapshot_db"))?;

    let state_bytes = rmp_serde::to_vec_named(&state.paradigm_state)
        .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;
    let last_applied_seq = state.total_events_applied;

    match crate::snapshot::write_instance_snapshot(
        event_store.as_ref(),
        db,
        &state.args.namespace,
        &state.args.instance_id,
        last_applied_seq,
        bytes::Bytes::from(state_bytes),
    )
    .await
    {
        Ok(result) => {
            tracing::info!(
                instance_id = %state.args.instance_id,
                seq = last_applied_seq,
                jetstream_seq = result.jetstream_seq,
                checksum = result.checksum,
                "snapshot written"
            );
            state.events_since_snapshot = 0;
        }
        Err(e) => {
            tracing::warn!(
                instance_id = %state.args.instance_id,
                error = %e,
                "snapshot write failed — continuing, will retry at next interval"
            );
        }
    }
    Ok(())
}
```

## Section 7: Contract (Pre/Post Conditions)

**Pre-conditions:**
- `state.args.snapshot_db` is `Some(sled::Db)` — instances without sled are in dev/test mode
- `state.args.event_store` is `Some(Arc<dyn EventStore>)` — required for `SnapshotTaken` event
- `state.paradigm_state` is `Serialize` — enforced by `#[derive(serde::Serialize)]` on `ParadigmState` (`lifecycle.rs:29`)
- `state.total_events_applied > 0` — at least one event was applied before triggering

**Post-conditions:**
- On success: `state.events_since_snapshot == 0`, sled contains `SnapshotRecord` under `instance_id` key, JetStream has `SnapshotTaken { seq, checksum }` event
- On failure: `state.events_since_snapshot` is NOT reset (retry at next interval), workflow continues (snapshot failure is non-fatal)
- `state.paradigm_state` is unchanged (snapshot is a write-aside, not destructive)

**Invariants:**
- `SnapshotRecord.seq` == `state.total_events_applied` at time of snapshot
- CRC32 checksum matches serialized bytes (computed by `SnapshotRecord::new`)
- Snapshots are idempotent: writing twice for same `instance_id` overwrites previous

## Section 8: Error Handling

| Error | Source | Severity | Action |
|-------|--------|----------|--------|
| `event_store` is `None` | Missing config | Error (stops message processing) | Log error, return `ActorProcessingErr` |
| `snapshot_db` is `None` | Missing sled config | Error (stops message processing) | Log error, return `ActorProcessingErr` |
| `rmp_serde` serialization failure | Unserializable state | Error (stops message processing) | Log error, return `ActorProcessingErr` |
| sled write failure | `write_snapshot` returns `Err` | Warn (non-fatal) | Log warn, do NOT reset counter |
| JetStream publish failure | `write_instance_snapshot` returns `Err` | Warn (non-fatal) | Log warn, do NOT reset counter |

Recovery: On any snapshot failure, the counter is NOT reset. At the next `SNAPSHOT_INTERVAL` events, snapshot will be retried. Worst case: full JetStream replay on crash recovery.

## Section 9: Data Flow

```
inject_event (handlers.rs:195)
  |
  v
state.paradigm_state.apply_event(event, seq, phase)
  |
  v
state.events_since_snapshot += 1
  |
  v [if >= SNAPSHOT_INTERVAL]
handle_snapshot_trigger(state)  <-- THIS BEAD
  |
  +-- rmp_serde::to_vec_named(&state.paradigm_state)  ->  Vec<u8>
  |
  +-- crate::snapshot::write_instance_snapshot(...)
  |       |
  |       +-- SnapshotRecord::new(seq, state_bytes)  ->  SnapshotRecord { seq, state_bytes, checksum, taken_at }
  |       |
  |       +-- vo_storage::snapshots::write_snapshot(db, instance_id, &record)  ->  sled insert + flush
  |       |
  |       +-- event_store.publish(namespace, instance_id, SnapshotTaken { seq, checksum })  ->  JetStream append
  |
  +-- state.events_since_snapshot = 0  (on success only)
```

## Section 10: Implementation Steps

1. **Change function signature**: `fn handle_snapshot_trigger` -> `async fn handle_snapshot_trigger`, return type `-> Result<(), ActorProcessingErr>`
2. **Extract event_store**: `state.args.event_store.as_ref().ok_or_else(...)`
3. **Extract snapshot_db**: `state.args.snapshot_db.as_ref().ok_or_else(...)`
4. **Serialize paradigm_state**: `rmp_serde::to_vec_named(&state.paradigm_state).map_err(...)`
5. **Capture last_applied_seq**: `let last_applied_seq = state.total_events_applied;`
6. **Call write_instance_snapshot**: pass event_store, db, namespace, instance_id, seq, bytes
7. **Handle success**: log info, reset `state.events_since_snapshot = 0`
8. **Handle failure**: log warn, do NOT reset counter
9. **Update call site**: `inject_event` at line 209 change `handle_snapshot_trigger(state)` to `handle_snapshot_trigger(state).await?`
10. **Run quality gates**: `cargo check -p vo-actor && cargo clippy -p vo-actor -- -D warnings`

## Section 11: Testing Strategy

**Unit tests** (in `handlers.rs` or a new `tests` module):

| Test | Setup | Assertion |
|------|-------|-----------|
| `snapshot_trigger_serializes_and_resets` | Mock instance with `snapshot_db`, `event_store`, 100 events applied | Counter reset to 0, `write_instance_snapshot` called |
| `snapshot_trigger_no_event_store` | `event_store: None` | Returns `ActorProcessingErr` |
| `snapshot_trigger_no_snapshot_db` | `snapshot_db: None` | Returns `ActorProcessingErr` |
| `snapshot_trigger_on_sled_failure` | Mock sled that returns error | Counter NOT reset, warn logged |
| `snapshot_trigger_serialization_fails` | `ParadigmState` with unserializable field (mock) | Returns `ActorProcessingErr` |

**Integration test** (separate, marked `ignore` — requires NATS):
- Full round-trip: write snapshot, read back, verify seq and state_bytes match.

## Section 12: Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Sled write latency spikes block event processing | Low | Medium | `write_instance_snapshot` is already non-blocking async; sled flush is bounded |
| msgpack serialization grows with state size | Medium | Low | State is bounded by workflow definition; no unbounded collections |
| Double snapshot on crash mid-write | Low | Low | Snapshots are idempotent (overwrite by key); JetStream replay is always correct |
| `inject_event` call site change breaks compilation | Low | Low | Single call site at line 209; trivial `.await?` addition |

## Section 13: Observability

**Logs produced:**
- `INFO` on successful snapshot: `instance_id`, `seq`, `jetstream_seq`, `checksum`, message "snapshot written"
- `WARN` on failure: `instance_id`, `error`, message "snapshot write failed — continuing, will retry at next interval"

**Existing metrics** (no changes needed):
- `total_events_applied` tracked in `InstanceState`
- `events_since_snapshot` tracked in `InstanceState`

**Distributed tracing:**
- Snapshot writes are self-contained; no span propagation needed beyond existing instance span.

## Section 14: Acceptance Criteria

1. `handle_snapshot_trigger` is `async` and returns `Result<(), ActorProcessingErr>`
2. On success: sled has a `SnapshotRecord` for the instance, JetStream has `SnapshotTaken` event, counter reset
3. On failure (sled or JetStream): counter NOT reset, workflow continues
4. Missing `event_store` or `snapshot_db`: returns error
5. `cargo clippy -p vo-actor -- -D warnings` passes
6. `cargo test -p vo-actor` passes (existing tests not broken)
7. No `unwrap` or `expect` in new code (matches project `deny` policy)

## Section 15: Rollback Plan

Revert the single commit. The stub is already safe — it resets the counter and logs. The system works correctly without snapshots (falls back to full JetStream replay). No data migration needed since snapshots are write-aside.

## Section 16: References

- **ADR-019**: Snapshot trigger every 100 events for bounded replay latency
- **ADR-015**: Write-ahead pattern (sled before JetStream)
- `crates/vo-actor/src/snapshot.rs:47-73` — `write_instance_snapshot` implementation
- `crates/vo-storage/src/snapshots.rs:76-95` — `write_snapshot` sled write
- `crates/vo-storage/src/snapshots.rs:40-58` — `SnapshotRecord::new` with CRC32
- `crates/vo-actor/src/instance/lifecycle.rs:29-35` — `ParadigmState` enum (derives `Serialize`)
- `crates/vo-actor/src/instance/state.rs:21` — `events_since_snapshot: u32` field
- `crates/vo-actor/src/instance/handlers.rs:193` — `SNAPSHOT_INTERVAL = 100` constant
- `crates/vo-actor/src/messages/instance.rs:29` — `snapshot_db: Option<sled::Db>` field
