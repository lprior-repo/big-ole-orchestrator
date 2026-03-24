# Implementation Summary: wtf-49tp

- **bead_id**: wtf-49tp
- **bead_title**: instance: Implement snapshot trigger
- **phase**: STATE-3
- **updated_at**: 2026-03-23T00:00:00Z

## Files Modified

### `crates/wtf-actor/src/instance/handlers.rs` (PRIMARY)
- **Lines 208-210**: Updated call site — `handle_snapshot_trigger(state)` → `handle_snapshot_trigger(state).await?`
- **Lines 215-263**: Replaced stub `fn handle_snapshot_trigger` with real `async fn handle_snapshot_trigger` implementation
- **Lines 265-463**: Added `#[cfg(test)] mod tests` with 5 unit tests, 3 mock types, 3 helper functions

### `crates/wtf-actor/src/instance/mod.rs` (ANCILLARY FIX)
- **Lines 21-47**: Updated pre-existing test module — added `MockOkEventStore` + `EmptyReplayStream` mocks and wired `event_store` + `snapshot_db` into `test_args()` helper so that `snapshot_resets_counter_at_interval` passes with the real implementation

## Implementation Details

### `handle_snapshot_trigger` (lines 215-263)
Replaced the synchronous stub with an async function that:
1. Extracts `event_store` and `snapshot_db` from state args, returning `ActorProcessingErr` if either is `None`
2. Serializes `state.paradigm_state` to msgpack via `rmp_serde::to_vec_named`
3. Calls `crate::snapshot::write_instance_snapshot` with all required parameters
4. On success: logs INFO with `instance_id`, `seq`, `jetstream_seq`, `checksum`; resets `events_since_snapshot = 0`
5. On failure: logs WARN with `instance_id`, `error`; does NOT reset counter (non-fatal, retries at next interval)
6. Always returns `Ok(())` for write failures (non-fatal), `Err` only for missing prerequisites

### Call site update (line 209)
Changed from `handle_snapshot_trigger(state)` to `handle_snapshot_trigger(state).await?` so that missing `event_store`/`snapshot_db` stops event processing (as specified in Section 8 error table).

## Constraint Adherence

| Constraint | Status | Evidence |
|---|---|---|
| Zero `unwrap()`/`expect()` in source | PASS | No unwrap/expect in lines 215-263 (implementation) |
| Zero panics | PASS | All error paths handled via `ok_or_else`/`map_err`/`match` |
| Data→Calc→Actions | PASS | State extraction (Data) → serialization (Calc) → write_instance_snapshot (Action) |
| Expression-based | PASS | `match` on result is the primary control flow |
| No files outside spec scope modified | NOTE | `mod.rs` updated to fix pre-existing test broken by behavioral change |

## Tests Written

| Test Name | Pass/Fail | What It Validates |
|---|---|---|
| `snapshot_trigger_no_event_store_returns_error` | PASS | `event_store: None` → returns `ActorProcessingErr`, counter NOT reset |
| `snapshot_trigger_no_snapshot_db_returns_error` | PASS | `snapshot_db: None` → returns `ActorProcessingErr`, counter NOT reset |
| `snapshot_trigger_success_resets_counter` | PASS | Both stores present → `Ok(())`, counter reset to 0 |
| `snapshot_trigger_failure_keeps_counter` | PASS | Failing event_store → `Ok(())` (non-fatal), counter NOT reset |
| `snapshot_trigger_preserves_paradigm_state` | PASS | `paradigm_state` serialized bytes identical before/after snapshot |

## cargo test Output

```
running 73 tests
test instance::handlers::tests::snapshot_trigger_no_event_store_returns_error ... ok
test instance::handlers::tests::snapshot_trigger_no_snapshot_db_returns_error ... ok
test instance::handlers::tests::snapshot_trigger_success_resets_counter ... ok
test instance::handlers::tests::snapshot_trigger_failure_keeps_counter ... ok
test instance::handlers::tests::snapshot_trigger_preserves_paradigm_state ... ok
test instance::tests::snapshot_resets_counter_at_interval ... ok
... (all 73 unit tests + 31 integration tests pass)
test result: ok. 73 passed; 0 failed; 0 ignored
```

## cargo clippy Output

`cargo clippy -p wtf-actor` — zero warnings in `handlers.rs`. All 5 warnings referencing `handlers.rs` are pre-existing (missing `# Errors` doc sections, unused `async`, etc.).

`cargo clippy -p wtf-actor -- -D warnings` — 4 pre-existing errors in `wtf-common` (`missing_errors_doc` lint on `to_msgpack`/`from_msgpack`/`try_new`). No new warnings or errors introduced.

## Acceptance Criteria Status

1. `handle_snapshot_trigger` is `async` and returns `Result<(), ActorProcessingErr>` — DONE
2. On success: sled has SnapshotRecord, JetStream has SnapshotTaken, counter reset — DONE (delegated to `write_instance_snapshot`)
3. On failure: counter NOT reset, workflow continues — DONE
4. Missing stores: returns error — DONE
5. `cargo clippy -p wtf-actor -- -D warnings` — pre-existing failures in wtf-common only
6. `cargo test -p wtf-actor` passes — DONE (73/73 unit, 31/31 integration)
7. No `unwrap` or `expect` in new code — DONE
