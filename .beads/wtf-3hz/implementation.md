# Implementation Summary — Bead vo-3hz: terminate_workflow Defect Repairs

## Defects Fixed

### D-03 (CRITICAL): `handle_cancel` was a no-op

**File:** `crates/vo-actor/src/instance/handlers.rs`

**Problem:** `handle_cancel` logged "cancellation requested" and returned `Ok(())` immediately, without stopping the actor. The HTTP caller received 204 but the workflow kept running — a semantic lie.

**Fix (3 changes):**
1. Added `myself_ref: ActorRef<InstanceMsg>` parameter to `handle_cancel`
2. Publishes `WorkflowEvent::InstanceCancelled { reason }` to the event store before stopping (for durability — follows the pattern in `handle_completed`/`handle_failed`)
3. Calls `myself_ref.stop(Some(reason))` after sending the success reply — actually stops the actor
4. Updated call site in `handle_msg` to pass `myself_ref`

**Pattern followed:** Matches the existing `procedural_utils.rs` `handle_completed`/`handle_failed` pattern exactly: publish event → reply → stop.

### D-04 (HIGH): Dead actor returns 500 instead of 404

**File:** `crates/vo-actor/src/master/handlers/terminate.rs`

**Problem:** If the instance actor died between `state.get()` and `call_cancel()`, the RPC would fail with `CallResult::SenderError`, which was mapped to `TerminateError::Failed(...)` → HTTP 500. The correct semantic is `TerminateError::NotFound` → HTTP 404, since the actor is no longer there.

**Fix (2 changes):**
1. Added `instance_id: &InstanceId` parameter to `call_cancel`
2. `Ok(CallResult::SenderError)` now maps to `TerminateError::NotFound(instance_id.clone())`
3. `Err(_)` (send failure — actor already dead) also maps to `TerminateError::NotFound(instance_id.clone())`

## Incidental Fixes (pre-existing, blocking compilation)

- `master/mod.rs:93`: Removed erroneous double-`Ok()` wrapper around `handle_get_status` return value
- `master/handlers/heartbeat.rs:32-39`: Scoped `MutexGuard` in a block instead of explicit `drop()` to satisfy `Send` bound across await points

## Constraint Adherence

| Constraint | Status |
|---|---|
| Zero `unwrap()`/`expect()` in non-test code | ✅ — none added or removed |
| Functions under 25 lines | ✅ — `handle_cancel` is 25 lines (was 13), `call_cancel` is 19 lines |
| Data→Calc→Actions | ✅ — pure error mapping in `call_cancel`, I/O in `handle_cancel` |
| Make illegal states unrepresentable | ✅ — `TerminateError::NotFound` correctly typed |
| Expression-based | ✅ — all logic uses `if let`, `match`, `map_err` |
| Clippy clean | ✅ — `cargo check` passes with no warnings |

## Verification

```
cargo check -p vo-actor    → Finished (0 errors, 0 warnings)
cargo test -p vo-actor     → 68 unit tests passed, 27 integration tests passed, 0 failed
```

## Changed Files

| File | Change |
|---|---|
| `crates/vo-actor/src/instance/handlers.rs` | D-03: `handle_cancel` now publishes `InstanceCancelled` event and calls `myself_ref.stop()` |
| `crates/vo-actor/src/master/handlers/terminate.rs` | D-04: `call_cancel` maps `SenderError` and send failure to `NotFound` |
| `crates/vo-actor/src/master/mod.rs` | Incidental: removed double-`Ok()` wrapper |
| `crates/vo-actor/src/master/handlers/heartbeat.rs` | Incidental: scoped MutexGuard for `Send` compliance |
