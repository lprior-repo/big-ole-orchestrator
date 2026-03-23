# Implementation Round 3 — Bead wtf-0qg (spawn_workflow)

## Summary

Fixed all 4 mandatory items from Round 3 black-hat review. All 99 tests pass (68 unit + 31 integration).

## Changes

### N-15 (HIGH): Event publish error logging in `handle_cancel`

**File:** `crates/wtf-actor/src/instance/handlers.rs` (lines 122-138)

The `let _ = store.publish(...)` on the cancellation event publish was already fixed in a prior edit (same bug class as N-09 which was fixed in Round 2). The fix adds structured error logging with `instance_id` and `error` fields at `error!` level.

### N-02 (HIGH): Integration test for `spawn_and_register`

**File:** `crates/wtf-actor/tests/spawn_workflow_test.rs` (NEW — 205 lines)

Created 4 integration tests that exercise the full MasterOrchestrator RPC path:

1. **`start_workflow_returns_instance_id`** — Spawns orchestrator, sends `StartWorkflow`, verifies `Ok(InstanceId)`.
2. **`duplicate_instance_id_returns_already_exists`** — Sends `StartWorkflow` twice with same ID, verifies `Err(AlreadyExists)`.
3. **`get_status_returns_snapshot_after_spawn`** — Spawns instance, sends `GetStatus`, verifies `Some(snapshot)` with correct fields.
4. **`get_status_returns_none_for_unknown_instance`** — Verifies `GetStatus` returns `None` for non-existent instance.

Required a mock `EventStore` + `ReplayStream` (`EmptyReplayStream` that immediately returns `TailReached`) because `WorkflowInstance.pre_start` requires an event store.

### Farley: `handle_heartbeat_expired` split (37 lines → 3 functions ≤25 lines each)

**File:** `crates/wtf-actor/src/master/handlers/heartbeat.rs`

Extracted two functions from `handle_heartbeat_expired`:

- **`check_recovery_preconditions(state, instance_id) -> Option<String>`** (12 lines) — Checks active-map and in-flight guard. Returns `Some(in_flight_key)` if recovery should proceed.
- **`attempt_recovery(myself, state, instance_id, in_flight_key)`** (19 lines) — Fetches metadata, builds recovery args, spawns the actor, cleans up in-flight key.
- **`handle_heartbeat_expired`** (5 lines) — Delegates to the two helpers above.

### Farley: `handle_start_workflow` param reduction (8 params → 3 params)

**Files:**
- `crates/wtf-actor/src/master/handlers/start.rs` — Introduced `StartWorkflowParams` struct grouping namespace, instance_id, workflow_type, paradigm, input, reply.
- `crates/wtf-actor/src/master/handlers/mod.rs` — Re-exported `StartWorkflowParams`.
- `crates/wtf-actor/src/master/mod.rs` — Updated call site to construct `StartWorkflowParams` before delegating.

`handle_start_workflow` now has 3 params: `(myself, state, params)`.

## Constraint Adherence

| Constraint | Status | Evidence |
|------------|--------|----------|
| Function ≤ 25 lines | ✅ | `handle_heartbeat_expired` = 5 lines, `handle_start_workflow` = 21 lines |
| Function ≤ 5 params | ✅ | `handle_start_workflow` = 3 params, `handle_heartbeat_expired` = 3 params |
| Zero unwrap/expect in production | ✅ | No new unwrap/expect in any production code |
| Zero mut in core logic | ✅ | No `mut` introduced in production code |
| Clippy clean | ✅ | No new warnings in changed files |
| Tests pass | ✅ | 99/99 tests pass (68 unit + 31 integration) |

## Files Changed

1. `crates/wtf-actor/src/instance/handlers.rs` — N-15 error logging (pre-existing fix verified)
2. `crates/wtf-actor/src/master/handlers/heartbeat.rs` — Split `handle_heartbeat_expired` into 3 functions
3. `crates/wtf-actor/src/master/handlers/start.rs` — Introduced `StartWorkflowParams`, reduced params
4. `crates/wtf-actor/src/master/handlers/mod.rs` — Re-exported `StartWorkflowParams`
5. `crates/wtf-actor/src/master/mod.rs` — Updated call site
6. `crates/wtf-actor/tests/spawn_workflow_test.rs` — NEW integration test file (4 tests)

FIXES APPLIED
