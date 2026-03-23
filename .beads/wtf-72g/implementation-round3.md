# Implementation Summary — Round 3 — wtf-72g (get_workflow handler)

**Date:** 2026-03-23  
**Defects addressed:** CRITICAL-01 (FATAL), DDD-01 (HIGH)  
**Status:** FIXES APPLIED

---

## CRITICAL-01 Fix: Test File Invisibility

**Problem:** `tests/unit/get_workflow_handler_test.rs` was a dead test file — invisible to Cargo because:
1. It lived in a `tests/unit/` subdirectory (Rust integration tests must be direct children of `tests/`)
2. It used `use crate::` imports (invalid for integration tests, which are external crates)
3. It was included via `include!()` in `lib.rs`, but that pattern makes `cargo test --test <name>` impossible

**Fix applied:**

1. **Created** `tests/get_workflow_handler_test.rs` — a proper Cargo integration test target
2. **Fixed imports:**
   - `use crate::handlers::workflow::get_workflow` → `use wtf_api::handlers::get_workflow` (uses re-export from `handlers/mod.rs` `pub use workflow::*`)
   - `use crate::types::{ApiError, V3StatusResponse}` → `use wtf_api::types::{ApiError, V3StatusResponse}`
3. **Removed** `include!()` for get_workflow from `lib.rs` (the `mod unit_get_workflow` block)
4. **Deleted** old `tests/unit/get_workflow_handler_test.rs`

**Verification:**
```
$ cargo test -p wtf-api --test get_workflow_handler_test
running 4 tests
test get_unknown_workflow_returns_404 ... ok
test get_existing_workflow_returns_200 ... ok
test get_workflow_timeout_returns_503_with_retry_after ... ok
test get_workflow_bad_path_returns_400 ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Note on terminate tests:** `tests/unit/terminate_handler_test.rs` has the same architectural issue (subdirectory + `crate::` imports) but works via `include!()` in `lib.rs`. It passes because the `include!()` embeds it as a unit test. Flagged for follow-up but not in scope for this bead.

---

## DDD-01 Fix: Retry-After Header on 503 Responses

**Problem:** All 503 responses in `workflow.rs` returned `SERVICE_UNAVAILABLE` without a `Retry-After` header, violating HTTP semantics for transient failures.

**Fix applied:** Added `[("Retry-After", "5")]` to all 503 responses in `workflow.rs`:

| Function | Response | Retry-After Added |
|----------|----------|-------------------|
| `map_start_error` | `AtCapacity` → 503 | Yes |
| `map_start_error` | `PersistenceFailed` → 503 | Yes |
| `map_status_result` | `GetStatusError::Timeout` → 503 | Yes |
| `map_terminate_result` | `TerminateError::Timeout` → 503 | Yes |
| `map_actor_error` | `CallResult::Timeout` → 503 | Yes |
| `map_actor_error` | `CallResult::SenderError` → 503 | Yes |
| `map_actor_error` | `MessagingErr::ChannelClosed` → 503 | Yes |
| `map_actor_error` | `MessagingErr::SendErr` → 503 | Yes |
| `replay_to` inline | `no_store` → 503 | Yes |
| `replay_to` inline | `no_db` → 503 | Yes |

**Implementation note:** Match arms with mixed tuple sizes (2-element vs 3-element) required changing return types from `impl IntoResponse` to `Response` and calling `.into_response()` on each arm to normalize the concrete type.

**Test updated:** `get_workflow_timeout_returns_503` → `get_workflow_timeout_returns_503_with_retry_after` now also asserts `Retry-After: 5` header is present.

---

## Files Changed

| File | Action |
|------|--------|
| `crates/wtf-api/tests/get_workflow_handler_test.rs` | **Created** — proper integration test |
| `crates/wtf-api/tests/unit/get_workflow_handler_test.rs` | **Deleted** — dead subdirectory copy |
| `crates/wtf-api/src/lib.rs` | **Modified** — removed `include!()` for get_workflow |
| `crates/wtf-api/src/handlers/workflow.rs` | **Modified** — Retry-After on all 503 responses |

---

## Constraint Adherence

| Constraint | Status |
|------------|--------|
| Zero `unwrap()`/`expect()` in production | PASS — only in `#[cfg(test)]` modules |
| All functions < 25 lines | PASS |
| All functions ≤ 5 params | PASS |
| Expression-based style | PASS |
| No panics in core paths | PASS |
| `cargo check -p wtf-api` | PASS — 0 errors |
| `cargo test -p wtf-api --test get_workflow_handler_test` | PASS — 4/4 tests |
| `cargo test -p wtf-api` (full suite) | PASS — 37 unit + 4 get_workflow + 5 validate = 46 pass (7 journal_test failures are pre-existing AGENTS.md known issue) |
