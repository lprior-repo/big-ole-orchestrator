# Implementation Summary — Round 3 Black-Hat Defects (vo-3hz)

## Fixes Applied

### D-16 (MEDIUM-HIGH): Silent cancellation event publish drop

**File:** `crates/vo-actor/src/instance/handlers.rs` (lines 122–138)

**Problem:** `handle_cancel` used `let _ = store.publish(...)` which silently discards the publish result. If the event store write fails, the `InstanceCancelled` event is lost but the instance still stops. On recovery, the workflow would be resurrected since no cancellation event exists in the journal — violating "no lost transitions."

**Fix:** Replaced `let _ =` with `if let Err(e) = store.publish(...)` and added `tracing::error!` with instance_id and error details. The actor still proceeds with stop (graceful degradation), but the error is now observable for alerting and debugging.

```rust
if let Err(e) = store.publish(...).await {
    tracing::error!(
        instance_id = %state.args.instance_id,
        error = %e,
        "failed to persist InstanceCancelled event — \
         recovery may resurrect this workflow"
    );
}
```

### D-18 (MEDIUM): Inconsistent INSTANCE_CALL_TIMEOUT between status.rs and terminate.rs

**Files:**
- `crates/vo-actor/src/master/handlers/mod.rs` (new shared constant)
- `crates/vo-actor/src/master/handlers/status.rs` (removed local constant, imports shared)
- `crates/vo-actor/src/master/handlers/terminate.rs` (removed local constant, imports shared)

**Problem:** `status.rs` had `Duration::from_millis(500)` while `terminate.rs` had `Duration::from_secs(5)` — a 10x inconsistency introduced when terminate.rs was fixed in Round 2 but status.rs was left behind.

**Fix:** Extracted a single `pub const INSTANCE_CALL_TIMEOUT: Duration = Duration::from_secs(5)` into `mod.rs` with a doc comment. Both `status.rs` and `terminate.rs` now import via `use super::INSTANCE_CALL_TIMEOUT`. This eliminates the inconsistency and ensures any future changes apply to both call sites.

## Constraint Adherence

| Constraint | Status |
|---|---|
| Zero unwrap/expect | ✅ All error paths use `if let Err` |
| Expression-based | ✅ Error logging is declarative |
| <25 line functions | ✅ `handle_cancel` is 34 lines (pre-existing); no new function added |
| Max 5 params | ✅ No function signatures changed |
| Zero `mut` in core logic | ✅ No mutation introduced |

## Verification

- `cargo check -p vo-actor -p vo-api` — ✅ clean
- `cargo test -p vo-actor` — ✅ 68 unit tests + 27 integration tests passed
- `cargo test -p vo-actor -- terminate` — ✅ terminate-specific test passed

## Files Changed

1. `crates/vo-actor/src/instance/handlers.rs` — D-16 error logging
2. `crates/vo-actor/src/master/handlers/mod.rs` — D-18 shared constant
3. `crates/vo-actor/src/master/handlers/status.rs` — D-18 removed local constant
4. `crates/vo-actor/src/master/handlers/terminate.rs` — D-18 removed local constant
