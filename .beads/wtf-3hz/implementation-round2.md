# Round 2 Implementation Summary — Bead vo-3hz (terminate_workflow)

## Fixes Applied

### D-01 (CRITICAL): HTTP Integration Tests for terminate_workflow

**File created:** `crates/vo-api/tests/unit/terminate_handler_test.rs`

4 HTTP integration tests following the `signal_handler_test.rs` mock-orchestrator pattern:

| Test | Scenario | Assertion |
|---|---|---|
| `terminate_existing_returns_204` | Mock replies `Ok(())` to Terminate | HTTP 204 NO_CONTENT |
| `terminate_unknown_returns_404` | Mock replies `TerminateError::NotFound` | HTTP 404, error code `"not_found"` |
| `terminate_bad_path_returns_400` | DELETE with no namespace slash | HTTP 400, error code `"invalid_id"` |
| `terminate_timeout_returns_503` | Mock replies `TerminateError::Timeout` | HTTP 503, error code `"instance_timeout"` |

Uses a dedicated `TerminateMock` actor (avoids name collision with `MockOrchestrator` from signal tests) in a separate `mod unit_terminate` in `lib.rs`.

### D-06 (HIGH): TerminateError::Timeout variant + 503 mapping

**Files modified:**
- `crates/vo-actor/src/messages/errors.rs` — Added `TerminateError::Timeout(InstanceId)` variant
- `crates/vo-actor/src/master/handlers/terminate.rs` — `CallResult::Timeout` now maps to `TerminateError::Timeout(instance_id.clone())` instead of `TerminateError::Failed("cancel timed out")`
- `crates/vo-api/src/handlers/workflow.rs` — `map_terminate_result` explicitly matches `TerminateError::Timeout` → HTTP 503 with `"instance_timeout"` code, and `TerminateError::Failed` → HTTP 500 with `"cancel_failed"` code (previously fell through to generic `map_actor_error`)

### D-10 (MEDIUM): INSTANCE_CALL_TIMEOUT increased to 5s

**File modified:** `crates/vo-actor/src/master/handlers/terminate.rs`

Changed `INSTANCE_CALL_TIMEOUT` from `Duration::from_millis(500)` to `Duration::from_secs(5)`, matching the orchestrator-level `ACTOR_CALL_TIMEOUT`. The cancel operation does I/O (event store publish + actor stop), so 500ms was too aggressive.

## Ancillary Fixes

- `crates/vo-api/src/handlers/workflow.rs` — Added missing `StartError::PersistenceFailed` match arm in `map_start_error` (pre-existing non-exhaustive pattern)
- `crates/vo-actor/src/master/handlers/list.rs` — Added missing `GetStatusError::ActorDied` match arm (pre-existing non-exhaustive pattern)
- `crates/vo-api/src/lib.rs` — Added `mod unit_terminate` with separate include for terminate test file

## Constraint Adherence

| Constraint | Status |
|---|---|
| Zero `unwrap()`/`expect()` outside tests | ✅ |
| Functions < 25 lines | ✅ |
| Functions < 5 params | ✅ |
| Explicit match on all variants | ✅ |
| No new `mut` in core logic | ✅ |
| Parse at boundary, trust in core | ✅ |

## Test Results

```
vo-actor:  terminate_returns_not_found_for_unknown_instance ... ok
vo-api:    terminate_existing_returns_204 ... ok
            terminate_unknown_returns_404 ... ok
            terminate_bad_path_returns_400 ... ok
            terminate_timeout_returns_503 ... ok
```

## Files Changed

1. `crates/vo-actor/src/messages/errors.rs` — Added `Timeout(InstanceId)` to `TerminateError`
2. `crates/vo-actor/src/master/handlers/terminate.rs` — Timeout 500ms→5s, map to `TerminateError::Timeout`
3. `crates/vo-actor/src/master/handlers/list.rs` — Fixed non-exhaustive match
4. `crates/vo-api/src/handlers/workflow.rs` — Explicit Timeout→503, Failed→500, PersistenceFailed arm
5. `crates/vo-api/src/lib.rs` — Added `mod unit_terminate` include
6. `crates/vo-api/tests/unit/terminate_handler_test.rs` — **NEW** 4 HTTP integration tests
