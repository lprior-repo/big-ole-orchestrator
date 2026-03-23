# Black Hat Review — Bead wtf-3hz: ROUND 4

**Reviewer:** Black Hat (glm-5-turbo)
**Date:** 2026-03-23
**Verdict:** APPROVED

---

## Round 3 Defect Verification

### D-16 (MEDIUM-HIGH): Silent cancellation event publish drop

**Status: FIXED** ✅

`handlers.rs:122-138` now wraps the publish call in `if let Err(e)` with a `tracing::error!` log that includes `instance_id` and `error` fields, plus a diagnostic message: `"failed to persist InstanceCancelled event — recovery may resurrect this workflow"`.

This is the correct fix. The actor still proceeds with `stop()` (graceful degradation), but the failure is now observable in logs. The error message explicitly warns operators about the resurrection risk. **Verified fix.**

### D-18 (MEDIUM): Inconsistent INSTANCE_CALL_TIMEOUT

**Status: FIXED** ✅

- `mod.rs:11`: Single definition `pub const INSTANCE_CALL_TIMEOUT: Duration = Duration::from_secs(5);`
- `status.rs:5`: `use super::INSTANCE_CALL_TIMEOUT;` — no local constant.
- `terminate.rs:6`: `use super::INSTANCE_CALL_TIMEOUT;` — no local constant.

Both files now reference the same 5-second constant. The doc comment on `mod.rs:10` explains the purpose. No duplication. **Verified fix.**

---

## Round 2 Defect Regression Check

| Defect | Status | Evidence |
|---|---|---|
| D-01 (CRITICAL): Zero HTTP tests | ✅ FIXED | `terminate_handler_test.rs`: 4 tests — 204, 404, 400, 503. All pass. |
| D-06 (HIGH): Timeout → 500 | ✅ FIXED | `TerminateError::Timeout(InstanceId)` variant exists (`errors.rs:25`). `terminate.rs:39` maps `CallResult::Timeout` → `Timeout`. `workflow.rs:150` maps → 503 + `Retry-After`. |
| D-09 (LOW): Failed(String) opaque | ✅ FIXED (by D-06) | `TerminateError::Timeout` now exists as distinct variant. `Failed(String)` remains for genuine cancel errors. |
| D-10 (MEDIUM): 500ms timeout | ✅ FIXED | Both `status.rs` and `terminate.rs` use `INSTANCE_CALL_TIMEOUT` = 5s (`mod.rs:11`). |
| D-03 (CRITICAL): handle_cancel no-op | ✅ STILL FIXED | `handlers.rs:110-143`: publishes event, sends reply, calls `stop()`. |
| D-04 (HIGH): SenderError → 500 | ✅ STILL FIXED | `terminate.rs:40-41`: SenderError → NotFound. |
| D-05 (LOW): Duplicate route | CARRIED | Pre-existing dead code in `routes.rs`. Not introduced by this bead. |
| D-02 (LOW): Hardcoded reason | CARRIED | Pre-existing design choice. Not a correctness issue. |
| D-08 (MEDIUM): Publish failure silent | ✅ FIXED (by D-16) | Now logged at `tracing::error!` level with resurrection warning. |

---

## PHASE 1: Contract & Bead Parity

### 1.1 HTTP Contract Compliance

| Requirement | Status | Evidence |
|---|---|---|
| `DELETE /api/v1/workflows/:id` → 204 | ✅ | `workflow.rs:148`: `StatusCode::NO_CONTENT` |
| `DELETE /api/v1/workflows/:id` → 404 | ✅ | `workflow.rs:149`: `TerminateError::NotFound` → `NOT_FOUND` |
| `DELETE /api/v1/workflows/:id` → 503 on timeout | ✅ | `workflow.rs:150`: `TerminateError::Timeout` → `SERVICE_UNAVAILABLE` + `Retry-After: 5` |
| `DELETE /api/v1/workflows/:id` → 400 bad path | ✅ | `workflow.rs:47`: `split_path_id` → `BAD_REQUEST` |
| `DELETE /api/v1/workflows/:id` → 500 on cancel failed | ✅ | `workflow.rs:151`: `TerminateError::Failed` → `INTERNAL_SERVER_ERROR` |
| Actor errors → 503 | ✅ | `workflow.rs:152`: `map_actor_error` → `SERVICE_UNAVAILABLE` + `Retry-After: 5` |

### 1.2 TerminateError Type Completeness

| Variant | Used In | HTTP Mapping |
|---|---|---|
| `NotFound(InstanceId)` | `terminate.rs:15,40-41` | 404 |
| `Timeout(InstanceId)` | `terminate.rs:39` | 503 + Retry-After |
| `Failed(String)` | `terminate.rs:37` | 500 |

All three variants are constructed in the actor layer and mapped in the HTTP layer. No dead variants. No unhandled match arms. ✅

### 1.3 Test Parity

| Test | What it asserts | Status |
|---|---|---|
| `terminate_existing_returns_204` | DELETE valid → 204 | ✅ |
| `terminate_unknown_returns_404` | DELETE nonexistent → 404 + `not_found` body | ✅ |
| `terminate_bad_path_returns_400` | DELETE no-slash → 400 + `invalid_id` body | ✅ |
| `terminate_timeout_returns_503` | DELETE timeout → 503 + `instance_timeout` body | ✅ |
| `terminate_returns_not_found_for_unknown_instance` (actor unit) | Orchestrator→NotFound for missing instance | ✅ |

5 tests. All pass. Contract parity achieved. ✅

---

## PHASE 2: Farley Engineering Rigor

### 2.1 Function Length (>25 lines)

| Function | File | Lines | Verdict |
|---|---|---|---|
| `handle_msg` | `handlers.rs:12-28` | 17 | ✅ |
| `handle_procedural_msg` | `handlers.rs:30-60` | 31 | ⚠️ WARNING |
| `handle_cancel` | `handlers.rs:110-143` | 34 | ⚠️ WARNING |
| `handle_terminate` | `terminate.rs:8-21` | 14 | ✅ |
| `call_cancel` | `terminate.rs:23-43` | 21 | ✅ |
| `handle_get_status` | `status.rs:7-21` | 15 | ✅ |
| `terminate_workflow` | `workflow.rs:41-53` | 13 | ✅ |
| `map_terminate_result` | `workflow.rs:146-154` | 9 | ✅ |
| `TerminateMock::handle` | `test:30-54` | 25 | ✅ |

`handle_procedural_msg` (31 lines) and `handle_cancel` (34 lines) exceed the 25-line soft limit. However, `handle_procedural_msg` is a pure match dispatch — no logic, just delegation to `procedural::*` functions. It's structurally equivalent to a table. `handle_cancel` is at 34 lines because the error logging block (D-16 fix) added 8 lines, but the function remains a single-responsibility block: validate → publish → reply → stop.

**Not blocking. These are over the line but not bloated.**

### 2.2 Parameter Count (>5 params)

All functions have ≤5 parameters. ✅

### 2.3 Functional Core / Imperative Shell

- Pure logic: `call_cancel`, `handle_terminate`, `map_terminate_result`, `map_actor_error` — all take inputs, return outputs, no side effects beyond the result type.
- Imperative shell: `handle_cancel` (publishes to event store, stops actor), `terminate_workflow` (calls actor via RPC).

Clean separation. The `map_*` functions are pure mappings from `CallResult` → HTTP `Response`. ✅

### 2.4 Test Quality

All 4 HTTP tests assert **behavior** (status codes, error codes in JSON body). None assert implementation details (no checking internal actor state, no inspecting private fields). The mock actor pattern is the correct level of abstraction — it mocks the actor boundary, not internal function calls. ✅

---

## PHASE 3: NASA-Level Functional Rust (The Big 6)

### 3.1 Illegal States Unrepresentable

✅ `TerminateError` is a 3-variant enum. Cannot construct a `Timeout` without an `InstanceId`. Cannot construct `NotFound` without an `InstanceId`. The `Failed` variant carries a `String` which is the only weak spot (carried forward D-09 — LOW, not blocking).

### 3.2 Parse, Don't Validate

✅ `split_path_id` at the HTTP boundary parses raw path into `(String, InstanceId)`. `InstanceId` is a newtype — trusted downstream. No re-validation.

### 3.3 Types as Documentation

`call_cancel(actor_ref: &ActorRef<InstanceMsg>, instance_id: &InstanceId, reason: String)` — all parameters are typed. No boolean flags. `reason: String` could be a newtype `CancelReason` for domain purity, but this is cosmetic at this layer. ✅

### 3.4 Workflows as Explicit State Transitions

Cancel flow is a clean message chain:
1. HTTP `DELETE` → `OrchestratorMsg::Terminate`
2. Orchestrator `handle_terminate` → `InstanceMsg::Cancel`
3. Instance `handle_cancel` → publish `InstanceCancelled` → reply `Ok(())` → `myself_ref.stop()`

Each transition is explicit in the message type. ✅

### 3.5 Newtypes

`InstanceId` wraps `String`. No raw `String` for IDs in domain code. ✅

### 3.6 No `unwrap()`/`expect()`/`panic!()`

Scanned the entire terminate call chain:
- `handlers.rs` handle_cancel: No unwrap/expect.
- `terminate.rs` handle_terminate/call_cancel: No unwrap/expect. Uses `let _ = reply.send(...)`.
- `workflow.rs` terminate_workflow/map_terminate_result: No unwrap/expect.
- `terminate_handler_test.rs` tests: `expect()` only in test setup (request builder, actor spawn) — acceptable.

✅ Zero panic vectors in production code.

---

## PHASE 4: Ruthless Simplicity & DDD (Scott Wlaschin)

### 4.1 CUPID Properties

| Property | Assessment |
|---|---|
| **Composable** | ✅ `map_terminate_result` composes with `map_actor_error` via `_ =>` fallthrough |
| **Unix-philosophy** | ✅ Each function does one thing. `handle_terminate` dispatches. `call_cancel` executes. `map_terminate_result` translates. |
| **Predictable** | ✅ Same input → same HTTP response. No hidden state. |
| **Idiomatic** | ✅ Standard Rust error handling with `Result`, `match`, `if let Err`. |
| **Domain-based** | ✅ `TerminateError` vocabulary matches the domain (NotFound, Timeout, Failed). |

### 4.2 No Option-Based State Machines

✅ No `Option<bool>` or `Option<T>` used to encode state transitions. The workflow state is in the `ParadigmState`, not scattered across options.

### 4.3 Panic Vector

✅ Zero `unwrap()` in production terminate path. Zero `expect()` in production terminate path. `let mut` only in test setup and one existing location (`handle_terminate` takes `&mut OrchestratorState` — required by the API design for `state.get()`).

---

## PHASE 5: The Bitter Truth (Velocity & Legibility)

### 5.1 Code is Boring

This is now boring, predictable code. A DELETE handler that calls an actor, maps three error variants to HTTP codes, and has 4 tests proving each mapping. No cleverness. No abstractions with one implementer. No "future-proofing." Exactly what a production terminate endpoint should look like.

### 5.2 YAGNI Compliance

No generic handlers. No abstract traits. `TerminateError` has exactly the three variants needed. The shared `INSTANCE_CALL_TIMEOUT` is used by exactly two files. No dead code in the terminate path itself.

### 5.3 The Sniff Test

Does this look like a junior dev trying to prove how smart they are? **No.** It looks like someone who got beaten up in 3 rounds of review and wrote the obvious, correct thing. That's exactly what I want.

### 5.4 Remaining Carried-Forward Items

| Item | Severity | Blocking? |
|---|---|---|
| D-05: Duplicate route registration in `routes.rs` | LOW | ❌ Pre-existing dead code, not introduced by this bead |
| D-02: Hardcoded `reason: "api-terminate"` | LOW | ❌ Design choice, not a correctness issue |
| D-09: `Failed(String)` opaque | LOW | ❌ D-06 fix added `Timeout` variant. `Failed` is only for genuine errors now. |

None blocking.

---

## Test Execution Verification

```
$ cargo test -p wtf-actor -- terminate
  test master::handlers::terminate::tests::terminate_returns_not_found_for_unknown_instance ... ok
  1 passed; 0 failed

$ cargo test -p wtf-api -- terminate
  test tests::unit_terminate::terminate_bad_path_returns_400 ... ok
  test tests::unit_terminate::terminate_existing_returns_204 ... ok
  test tests::unit_terminate::terminate_timeout_returns_503 ... ok
  test tests::unit_terminate::terminate_unknown_returns_404 ... ok
  4 passed; 0 failed
```

All 5 tests (1 actor unit + 4 HTTP) pass. ✅

---

## Defect Summary

| ID | Phase | Severity | Status |
|---|---|---|---|
| D-16 | Functional Rust | MEDIUM-HIGH | ✅ FIXED — Event publish failure now logged |
| D-18 | Bitter Truth | MEDIUM | ✅ FIXED — Shared `INSTANCE_CALL_TIMEOUT` = 5s in `mod.rs` |
| D-01 | Farley | CRITICAL | ✅ FIXED (Round 3) — 4 HTTP tests |
| D-06 | Contract | HIGH | ✅ FIXED (Round 3) — `TerminateError::Timeout` → 503 |
| D-09 | DDD | LOW | ✅ FIXED (by D-06) |
| D-10 | Bitter Truth | MEDIUM | ✅ FIXED (by D-18) |
| D-03 | Contract | CRITICAL | ✅ FIXED (Round 2) |
| D-04 | Contract | HIGH | ✅ FIXED (Round 2) |
| D-08 | Functional Rust | MEDIUM | ✅ FIXED (by D-16) |
| D-05 | Bitter Truth | LOW | CARRIED — Pre-existing dead code |
| D-02 | Functional Rust | LOW | CARRIED — Design choice |

### New Defects Found This Round

**None.**

---

## Final Verdict

**STATUS: APPROVED**

Four rounds. The code is clean. The terminate endpoint has:
- A 3-variant `TerminateError` enum that makes illegal states unrepresentable
- Proper HTTP mapping: 204/400/404/500/503 with appropriate `Retry-After` headers
- 5 tests (1 actor unit + 4 HTTP integration) that all pass
- A shared 5-second timeout constant used consistently across status and terminate
- Observable error logging for event store publish failures
- Zero `unwrap()`/`expect()` in production code
- No functions over the parameter limit
- Clean functional core / imperative shell separation

The only remaining items (D-02, D-05) are pre-existing, LOW severity, and not introduced by this bead. They are tracked but do not block.

Ship it.
