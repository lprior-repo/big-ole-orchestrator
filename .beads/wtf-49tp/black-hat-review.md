# Black Hat Review — wtf-49tp: "instance: Implement snapshot trigger"

Reviewer: Black Hat (adversarial audit)
Date: 2026-03-23

---

## 1. Hallucinated APIs — `write_instance_snapshot` signature

**VERIFIED — NO DEFECT.**

Call site (`handlers.rs:281-289`):
```rust
crate::snapshot::write_instance_snapshot(
    event_store.as_ref(),   // &dyn EventStore
    db,                     // &sled::Db
    &state.args.namespace,  // &NamespaceId
    &state.args.instance_id,// &InstanceId
    last_applied_seq,       // u64
    Bytes::from(state_bytes), // Bytes
)
```

Actual signature (`snapshot.rs:47-54`):
```rust
pub async fn write_instance_snapshot(
    event_store: &dyn EventStore,
    db: &sled::Db,
    namespace: &NamespaceId,
    instance_id: &InstanceId,
    last_applied_seq: u64,
    state_bytes: Bytes,
) -> Result<SnapshotResult, WtfError>
```

All 6 parameters match in type and order. The `SnapshotResult` fields
(`jetstream_seq`, `checksum`) used at `handlers.rs:295-296` exist in the struct
(`snapshot.rs:29-31`).

The `event_store` is `Arc<dyn EventStore>` (verified in `messages/instance.rs:23`);
`.as_ref()` correctly yields `&dyn EventStore`.

## 2. Silent Failures — msgpack serialization → ActorProcessingErr

**VERIFIED — NO DEFECT.**

`handlers.rs:277-278`:
```rust
let state_bytes = rmp_serde::to_vec_named(&state.paradigm_state)
    .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;
```

Serialization error is mapped to `ActorProcessingErr` via `Box<dyn Error>`.
The `?` propagates it — this is a hard failure path (prevents stale snapshot).

## 3. Contract Violations — `events_since_snapshot` reset

**VERIFIED — NO DEFECT.**

`handlers.rs:291-308`: Counter is ONLY reset to 0 inside the `Ok(result)` branch
(line 299). On `Err(e)` (line 301-307), counter is untouched. This matches ADR-019
and is validated by tests:
- `snapshot_trigger_success_resets_counter` (line 462)
- `snapshot_trigger_failure_keeps_counter` (line 477)
- `snapshot_trigger_no_event_store_returns_error` (line 434)
- `snapshot_trigger_no_snapshot_db_returns_error` (line 448)

## 4. Dead Code / Unreachable Branches

**VERIFIED — NO DEFECT.**

`persist_local_snapshot` (`snapshot.rs:75-83`) logs a warning on sled failure but
does NOT error — this is intentional (comment says "recovery will replay from start").
The function returns `()`, so there's no unreachable `Err` branch to handle.

`should_snapshot` (`snapshot.rs:114-116`) is used by the `>=` check in `inject_event`
(`handlers.rs:256`) but also exported for testing. Not dead.

## 5. Type Safety — unsafe casts / transmutes

**VERIFIED — NO DEFECT.**

`snapshot.rs:17` declares `#![forbid(unsafe_code)]`. No `unsafe` blocks anywhere in
either file. No transmutes or raw pointer casts.

---

## Observations (non-blocking)

1. **Dual logging at same level**: `handle_snapshot_trigger` logs `"snapshot written"`
   at `info!` (handlers.rs:292), and `publish_snapshot_event` also logs
   `"snapshot written"` at `debug!` (snapshot.rs:99). Not a defect, but slightly
   confusing — the inner log is redundant when the outer exists.

2. **`persist_local_snapshot` swallows sled errors**: If sled write fails, the
   function logs a warning but continues to publish the JetStream `SnapshotTaken`
   event. This means recovery may see a `SnapshotTaken` event pointing to a seq
   for which no sled snapshot exists. The fallback (full replay) handles this
   correctly, but the asymmetry is worth noting for ops alerting.

---

## Verdict

**STATUS: APPROVED**

All five audit dimensions pass. No hallucinated APIs, no silent failures,
no contract violations, no dead code, no unsafe operations. The implementation
correctly follows ADR-019 and has thorough test coverage for both success and
failure paths.
