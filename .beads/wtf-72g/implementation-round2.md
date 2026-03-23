# Implementation Summary — Round 2 Fixes for wtf-72g

**Date:** 2026-03-23  
**Defects Addressed:** DEFECT-04, DEFECT-R2-01, DEFECT-01  
**Status:** FIXES APPLIED

---

## DEFECT-R2-01 (CRITICAL): Expand GetStatusError — Timeout vs ActorDied

### Problem
`GetStatusError` was a single-variant enum (`Timeout` only). The `_` wildcard in `status.rs` collapsed `CallResult::Timeout`, `CallResult::SenderError`, and `Err(_)` into one meaningless variant. A dead actor (permanent) was indistinguishable from a slow actor (transient), misleading clients into retrying ghost instances.

### Changes

**`crates/wtf-actor/src/messages/errors.rs`** — Added `ActorDied` variant:
```rust
pub enum GetStatusError {
    #[error("instance actor timed out")]
    Timeout,
    #[error("instance actor died or was killed")]
    ActorDied,
}
```

**`crates/wtf-actor/src/master/handlers/status.rs`** — Replaced `_` wildcard with explicit matching:
- `Ok(CallResult::Success(snapshot))` → `Ok(Some(snapshot))`
- `Ok(CallResult::Timeout)` → `Err(GetStatusError::Timeout)`
- `Ok(CallResult::SenderError)` → `Err(GetStatusError::ActorDied)`
- `Err(_)` → `Err(GetStatusError::ActorDied)`

**`crates/wtf-api/src/handlers/workflow.rs`** — Updated `map_status_result`:
- `GetStatusError::Timeout` → 503 SERVICE_UNAVAILABLE (transient, retry is reasonable)
- `GetStatusError::ActorDied` → 404 NOT_FOUND (permanent, no point retrying)

### Constraint Verification
- Zero `unwrap`/`expect` in handlers
- Explicit pattern matching — no wildcards for error variants
- Illegal states made unrepresentable: type system forces callers to handle both error kinds

---

## DEFECT-04 (CRITICAL): HTTP integration tests for get_workflow

### Problem
Zero HTTP tests existed for `get_workflow`. The `signal_handler_test.rs` scaffold existed but was never replicated.

### Changes

**`crates/wtf-api/tests/unit/get_workflow_handler_test.rs`** — New file with 4 tests:
1. **`get_existing_workflow_returns_200`** — Mock returns `Some(snapshot)` → asserts 200 + JSON body with correct field values
2. **`get_unknown_workflow_returns_404`** — Mock returns `None` → asserts 404 + `ApiError` with `error: "not_found"`
3. **`get_workflow_bad_path_returns_400`** — No `/` in path → asserts 400 + `ApiError` with `error: "invalid_id"`
4. **`get_workflow_timeout_returns_503`** — Mock drops reply → asserts 503

**`crates/wtf-api/src/lib.rs`** — Added `mod unit_get_workflow` include to wire the test into the test harness.

### Constraint Verification
- Uses proven `MockOrchestrator` + `Router::new()` + `oneshot()` pattern
- Shared `test_snapshot()` factory for deterministic test data
- Shared `build_app()` builder for DRY test setup
- All assertions check both status code AND response body

---

## DEFECT-01 (HIGH): Kill dead code — WorkflowStatus and ListWorkflowsResponse

### Problem
Three structurally identical status types existed: `WorkflowStatus`, `V3StatusResponse`, and `InstanceStatusSnapshot`. `WorkflowStatus` was only referenced by `ListWorkflowsResponse.workflows`, which itself was never used by any handler. Dead code.

### Changes

**`crates/wtf-api/src/types/responses.rs`** — Removed:
- `WorkflowStatus` struct (was lines 37-46)
- `ListWorkflowsResponse` struct (was lines 88-92)

### Verification
- `cargo check -p wtf-api` passes — no references remain
- `V3StatusResponse` is the single canonical status response type

---

## Files Changed

| File | Change |
|------|--------|
| `crates/wtf-actor/src/messages/errors.rs` | Added `GetStatusError::ActorDied` variant |
| `crates/wtf-actor/src/master/handlers/status.rs` | Explicit match on all `CallResult` variants |
| `crates/wtf-api/src/handlers/workflow.rs` | `map_status_result` handles `ActorDied` → 404 |
| `crates/wtf-api/src/types/responses.rs` | Deleted `WorkflowStatus` + `ListWorkflowsResponse` |
| `crates/wtf-api/tests/unit/get_workflow_handler_test.rs` | **NEW** — 4 HTTP handler tests |
| `crates/wtf-api/src/lib.rs` | Wired `get_workflow_handler_test.rs` into test harness |

---

## Test Results

```
wtf-actor:  68 unit tests PASSED
wtf-api:    41 lib tests PASSED (including 4 new get_workflow tests)
cargo check: CLEAN (wtf-actor + wtf-api)
```

Pre-existing journal_test failures (7) are unrelated — documented in AGENTS.md.
