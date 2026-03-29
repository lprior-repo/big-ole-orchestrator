# Black Hat Review — Round 3 — vo-72g (get_workflow handler)

**Reviewer:** Black Hat  
**Date:** 2026-03-23  
**Files inspected:** 5  

---

## R2 Fix Verification

| Fix | Status | Evidence |
|-----|--------|----------|
| R2-01: GetStatusError has Timeout + ActorDied | **PASS** | `errors.rs:32-37` — two explicit variants present |
| R2-01: status.rs explicit matching | **PASS** | `status.rs:17-22` — no wildcards, explicit `CallResult::Timeout` → `Timeout`, `SenderError`/`Err` → `ActorDied` |
| R2-01: map_status_result handles both | **PASS** | `workflow.rs:136-141` — `Timeout` → 503, `ActorDied` → 404, explicit arms |
| DEFECT-01: WorkflowStatus deleted | **PASS** | `grep` returns zero hits across entire workspace |
| DEFECT-01: ListWorkflowsResponse deleted | **PASS** | `grep` returns zero hits across entire workspace |
| cargo check passes | **PASS** | Clean build, 0 errors |

---

## PHASE 1: Contract & Bead Parity

### CRITICAL-01: Tests Are Dead Code — They Cannot Compile (FATAL)

`get_workflow_handler_test.rs` lives at `tests/unit/get_workflow_handler_test.rs`. This is a **subdirectory** of `tests/`. Rust integration tests require files to be **direct children** of `tests/` (each `.rs` file becomes a separate test crate). Files in `tests/unit/` are never discovered.

**Proof:**
```
$ cargo test -p vo-api --test get_workflow_handler_test
error: no test target named `get_workflow_handler_test` in `vo-api` package
help: available test targets: journal_test, validate_workflow_test
```

There is no `tests/unit/mod.rs` file. There is no `[[test]]` entry in `Cargo.toml`. The test is **completely invisible to Cargo**.

**Secondary defect in same file — `use crate::` paths (lines 14-15):**
```rust
use crate::handlers::workflow::get_workflow;
use crate::types::{ApiError, V3StatusResponse};
```

Even if the file were moved to `tests/`, integration tests cannot use `crate::` — they're external crates. These must be `vo_api::handlers::workflow::get_workflow` and `vo_api::types::{ApiError, V3StatusResponse}`. The `handlers` and `types` modules would need to be `pub`.

**This means DEFECT-04 is NOT fixed.** Tests exist on disk but are structurally non-functional. Zero tests actually run.

**Verdict:** REJECT. Fix both issues: move file to `tests/get_workflow_handler_test.rs` (or add proper module structure), and fix all imports.

---

## PHASE 2: Farley Engineering Rigor

### FARLEY-01: `workflow.rs` at 288 lines — Danger Zone

File is at 288 lines. Under the 300-line ceiling, but only because the handler is co-located with 4 other handlers (`start_workflow`, `terminate_workflow`, `list_workflows`, `replay_to`) plus their mapper functions and a full `do_replay_to`/`load_snapshot` subsystem. This file mixes HTTP handlers, business-logic mappers, replay orchestration, and a test module. It is one new handler away from violating the hard limit.

**Recommendation:** Split `replay_to` + its helpers into a separate file before it explodes.

### FARLEY-02: `do_replay_to` is 15 lines, `map_status_result` is 19 lines — PASS

All functions are under 25 lines.

### FARLEY-03: `load_snapshot` has 14 parameters via a mega-struct literal (line 217-222) — Note

`InstanceArguments` is a 10-field struct constructed inline. Not a function parameter violation, but worth flagging: if any new field is added to `InstanceArguments`, every call site becomes a maintenance burden.

### FARLEY-04: Test assertions test behavior, not implementation — PASS

Tests assert HTTP status codes and JSON response body fields. Good.

### FARLEY-05: MockOrchestrator swallows all other messages (line 65: `_ => {}`) — Minor

The mock silently discards any non-`GetStatus` messages. This is acceptable for a focused unit test but could mask bugs if the handler is refactored to send different messages.

---

## PHASE 3: NASA-Level Functional Rust (The Big 6)

### RUST-01: `map_actor_error` uses wildcard `_ =>` arm (line 178) — Existing Issue

The author explicitly matched `GetStatusError` variants in `map_status_result` (good), but `map_actor_error` still has a `_ => (500, ...)` catch-all at line 178. This is used by `list_workflows`, `map_start_result`, and `map_terminate_result`. If `MessagingErr` gains a new variant, this silently collapses to 500 instead of forcing a compile error.

**Pre-existing. Not a regression. Flagged for follow-up.**

### RUST-02: No `unwrap()` in production code paths — PASS

`workflow.rs` handlers use zero `unwrap()`/`expect()` in production paths. The only `expect()` calls are in the `#[cfg(test)]` module (lines 233, 247) which is acceptable.

### RUST-03: `V3StatusResponse` uses raw `String` fields — Flag

`V3StatusResponse` (responses.rs:112-120) has `pub instance_id: String`, `pub namespace: String`, `pub workflow_type: String`. These are DTOs, not domain types, so bare strings are tolerable for serialization. However, `paradigm: String` and `phase: String` are stringly-typed when `WorkflowParadigm` and `InstancePhaseView` are proper enums in the domain.

**Not a regression. Flagged for follow-up.**

### RUST-04: No boolean parameters — PASS

No boolean parameters found in any reviewed function.

---

## PHASE 4: Ruthless Simplicity & DDD (Scott Wlaschin)

### DDD-01: `ErrorResponse` exists but is never used by `get_workflow` — Note

The codebase has a proper `ErrorResponse` struct with `RetryAfterSeconds` invariant enforcement (responses.rs:139-168), but `map_status_result` returns raw `(StatusCode, Json(ApiError))` tuples instead. The 503 timeout response (line 137) does not include a `Retry-After` header, despite the infrastructure existing.

**This was flagged in Round 2 as "no Retry-After." Still unfixed.** The `ErrorResponse` type with `RetryAfterSeconds` validation was built but never wired into the 503 path.

### DDD-02: `ApiError` and `ErrorResponse` are both error response types — Flag

Two error response structs coexist: `ApiError` (line 122-137, used by all handlers) and `ErrorResponse` (line 139-168, with retry-after support, used by nobody). This is type duplication. `ApiError` is the one that's actually used. `ErrorResponse` is dead weight unless something references it.

Let me verify:

`ErrorResponse` is used in `StartWorkflowResponse`'s validator scope and `JournalResponse` — actually, checking the grep results, `ErrorResponse` is only defined in responses.rs. Let me check if anything imports it:

(Determined from review: `ErrorResponse` has a `validate()` method and `RetryAfterSeconds` invariant, but no handler actually returns it. It's dead code.)

### DDD-03: `WorkflowStatusValue` enum (responses.rs:9-17) — Potential Dead Code

Still exists. Used by `StartWorkflowResponse.status`. If `StartWorkflowResponse` isn't wired into any handler, this is dead code too. Not directly related to `get_workflow` bead, but flagged.

---

## PHASE 5: The Bitter Truth

### BITTER-01: The tests are theater

165 lines of test code that do not compile, do not run, and would not pass import resolution if they did. The author wrote test assertions, built a mock actor, wrote four test functions — and none of it is reachable by `cargo test`. This is worse than zero tests. Zero tests is honest. Non-compiling tests are a lie that looks like coverage.

### BITTER-02: The 503 path has no Retry-After

Round 2 flagged this. The response is `SERVICE_UNAVAILABLE` with `"instance actor timed out"`. Per HTTP semantics, 503 SHOULD include `Retry-After`. The project has the type (`RetryAfterSeconds`) and the infrastructure (`ErrorResponse`). But the handler ignores it and returns `ApiError` instead.

### BITTER-03: `workflow.rs` is 288 lines and still accumulating

The replay-to feature (lines 66-223) is a completely separate concern from CRUD workflow operations. It accounts for ~160 lines. If this file gains one more handler or helper, it crosses the 300-line hard limit. Split it now, not after the violation.

### BITTER-04: `list_workflows` handler uses collapsed error mapping

Line 62: `_ => map_actor_error(res).into_response()`. The `CallResult::Success(Ok(snapshots))` case is matched, then everything else is collapsed to `map_actor_error`. This means if the orchestrator returns `CallResult::Timeout` or `CallResult::SenderError` for a list request, they all get the same generic 503/500 treatment. No domain-specific error handling.

---

## Summary Table

| ID | Severity | Phase | Description |
|----|----------|-------|-------------|
| CRITICAL-01 | **FATAL** | 1 | Tests in `tests/unit/` — invisible to Cargo. Zero tests actually run. Also uses invalid `use crate::` imports. |
| DDD-01 | HIGH | 4 | 503 timeout response has no `Retry-After` header despite infrastructure existing |
| DDD-02 | MEDIUM | 4 | `ErrorResponse` dead code — defined but no handler uses it |
| FARLEY-01 | MEDIUM | 2 | `workflow.rs` at 288 lines, one feature away from hard limit violation |
| BITTER-03 | MEDIUM | 5 | `replay_to` subsystem (~160 lines) should be in its own file |
| RUST-01 | LOW | 3 | `map_actor_error` has wildcard `_ =>` — pre-existing, not regression |
| DDD-03 | LOW | 4 | `WorkflowStatusValue` potentially dead code |
| BITTER-04 | LOW | 5 | `list_workflows` collapsed error mapping |

---

## Mandatory Fix List (Before Re-review)

1. **CRITICAL-01:** Move `get_workflow_handler_test.rs` from `tests/unit/` to `tests/`. Fix all `crate::` imports to `vo_api::`. Ensure the `handlers` module is `pub` (or re-export `get_workflow`). Verify `cargo test -p vo-api --test get_workflow_handler_test` compiles and all 4 tests pass.
2. **DDD-01:** Wire `Retry-After` header into the 503 timeout response, or delete `ErrorResponse`/`RetryAfterSeconds` as YAGNI dead code.

---

## Verdict

**STATUS: REJECTED**

The tests are structurally non-functional. They exist on disk but Cargo cannot find them, they use invalid import paths, and not a single assertion has ever executed. DEFECT-04 ("4 HTTP integration tests written") is a fiction — the tests were written but never wired. This is exactly the kind of lazy "close the ticket" behavior the Black Hat exists to catch.

Fix CRITICAL-01, prove it compiles and passes with `cargo test`, then request Round 4.
