## Summary

Implement snapshot persistence in `vo-storage` and snapshot-aware replay in the current `vo-actor` instance replay path.

## Source ADRs

- `docs/adr/v2/ADR-016-v2-atomic-storage-snapshots.md`
- `docs/adr/v2/ADR-020-v2-fjall-key-encoding.md`

## Scope

- Add a snapshots partition keyed by `[instance_id_16_bytes | sequence_u64_be]`.
- Implement `snapshot_write(keyspace, instance_id, sequence, state)` using deterministic serialization.
- Implement `snapshot_load_latest(keyspace, instance_id) -> Result<Option<(u64, InstanceState)>, StorageError>`.
- Snapshot every 100 events.
- Update the current instance replay implementation to load the latest snapshot first and replay only events with sequence greater than the snapshot sequence.

## Constraints

- Do not replay from sequence `1` when a valid snapshot exists.
- Do not write a snapshot on every event.
- Keep key ordering big-endian so reverse scans return the latest snapshot.
- Use current workspace crates and paths only.

## Relevant Files

- `crates/vo-storage/src/lib.rs`
- `crates/vo-storage/src/append.rs`
- `crates/vo-actor/src/instance_actor.rs`

## Acceptance

- Writing and then loading the latest snapshot returns the expected state.
- If snapshots exist at sequences 100 and 200, the latest load returns sequence 200.
- Replay with a snapshot skips pre-snapshot events.
- Loading from an empty snapshot partition returns `None`.
