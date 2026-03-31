# Black Hat Review — Round 3 — wtf-3hz (terminate_workflow handler)

**Reviewer:** Black Hat
**Date:** 2026-03-23
**Rounds reviewed:** 1, 2, 3

---

## Round 2 Defect Resolution Audit

| ID  | Description                            | Status  | Verdict |
|-----|----------------------------------------|---------|---------|
| D-01| HTTP integration tests (204/404/400/503)| FIXED   | Verified — 4 tests exist and PASS |
| D-06| TerminateError::Timeout → 503          | FIXED   | Verified — variant exists, mapped correctly |
| D-10| INSTANCE_CALL_TIMEOUT 500ms→5s         | FIXED   | Verified in terminate.rs line 8 |

---

## PHASE 1: Contract & Bead Parity

**D-01 Verification (HTTP Tests):**
- `terminate_handler_test.rs`: 129 lines, 4 tests.
- Tests: `terminate_existing_returns_204`, `terminate_unknown_returns_404`, `terminate_bad_path_returns_400`, `terminate_timeout_returns_503`.
- All 4 tests PASS (confirmed via `cargo test -p wtf-api -- "terminate"`).
- Pattern matches `signal_handler_test.rs`: mock actor, oneshot Router, `axum::body::to_bytes`, `ApiError` deserialization.

**PASS.** Tests exist, pass, and follow established patterns.

**D-06 Verification (Timeout → 503):**
- `TerminateError::Timeout(InstanceId)` exists in `errors.rs` line 25.
- `call_cancel` in `terminate.rs` line 41: `Ok(CallResult::Timeout) => Err(TerminateError::Timeout(instance_id.clone()))`.
- `map_terminate_result` in `workflow.rs` line 150: maps `TerminateError::Timeout` → `503 SERVICE_UNAVAILABLE` with `"instance_timeout"`.

**PASS.** Full chain verified: actor timeout → TerminateError::Timeout → 503.

**D-10 Verification (5s timeout):**
- `terminate.rs` line 8: `const INSTANCE_CALL_TIMEOUT: Duration = Duration::from_secs(5);`.

**PASS.**

---

## PHASE 2: Farley Engineering Rigor

### Function Sizes

| Function | File | Lines | Limit | Verdict |
|----------|------|-------|-------|---------|
| `handle_terminate` | terminate.rs | 10 | 25 | OK |
| `call_cancel` | terminate.rs | 16 | 25 | OK |
| `map_terminate_result` | workflow.rs | 8 | 25 | OK |
| `terminate_workflow` (handler) | workflow.rs | 13 | 25 | OK |
| `handle_cancel` | handlers.rs | 25 | 25 | **MARGINAL** — exactly at limit |
| `do_replay_to` | workflow.rs | 13 | 25 | OK |
| `load_snapshot` | workflow.rs | 13 | 25 | OK |

**DEFECT D-15 (INFO):** `handle_cancel` at exactly 25 lines is at the boundary. Not a violation, but worth noting.

### Parameter Counts

No function exceeds 5 parameters. `handle_terminate` has 4, `call_cancel` has 3. **PASS.**

### Functional Core / Imperative Shell

`handle_terminate` and `call_cancel` are pure orchestration — they call actors and map results. No I/O hidden inside pure logic. **PASS.**

### Test Quality

Tests assert **behavior** (status codes, error codes from deserialized JSON), not implementation details. The 404 test verifies `err.error == "not_found"` and the 503 test verifies `err.error == "instance_timeout"`. **PASS.**

---

## PHASE 3: NASA-Level Functional Rust (The Big 6)

### Make Illegal States Unrepresentable

`TerminateError` is a proper enum with 3 variants covering all failure modes. `call_cancel` maps every `CallResult` branch explicitly — no wildcards that could silently swallow new variants. **PASS.**

### Parse, Don't Validate

`split_path_id` returns `Option` — failure is handled at the boundary (handler level) with `match`. The terminate handler does not accept raw strings into the domain. **PASS.**

### Types as Documentation

`TerminateError::Timeout(InstanceId)` carries the instance ID — clear semantics. No boolean parameters detected. **PASS.**

### Workflows as State Transitions

Cancel is a fire-and-forget event publish + actor stop. This is a reasonable one-shot transition (Running → Terminating → Stopped). **PASS.**

### Newtypes

`InstanceId` is used throughout. `reason: String` for the cancel reason is fine — it's a freeform human-readable string, not a domain identifier. **PASS.**

---

## PHASE 4: Strict DDD & Panic Vector

### Panic Vector Scan

**terminate.rs:**
- No `unwrap()`, `expect()`, `panic!()`.

**workflow.rs:**
- `terminate_workflow`: No panics.
- `map_terminate_result`: No panics. All branches covered with `_ => map_actor_error(res)`.

**handlers.rs (instance):**
- `handle_cancel`: No `unwrap()`, no `expect()`. Uses `let _ = reply.send(...)` and `let _ = store.publish(...)`. **PASS.**

### `let _ =` (discarded results)

**DEFECT D-16 (MEDIUM):** `handlers.rs` line 124-129 — `handle_cancel` discards the result of `store.publish()`:
```rust
let _ = store.publish(
    &state.args.namespace,
    &state.args.instance_id,
    event,
).await;
```
The `InstanceCancelled` event is published to the event store, but failure is silently swallowed. If the event store is down, the client gets `Ok(())` and the actor stops, but **no cancellation event is recorded**. This means:
1. Recovery replay will not know this instance was cancelled.
2. The system's guarantee of "no lost transitions" is violated.

This was flagged in Round 1 as "handle_cancel no-op" and was acknowledged. However, the fix applied (publishing the event) **silently drops the publish result**. The event is ATTEMPTED but not GUARANTEED.

**SEVERITY: MEDIUM-HIGH** — This violates the core invariant of the wtf-engine ("guaranteed no lost transitions"). The event store publish MUST succeed before acknowledging cancellation, OR the actor must not stop.

### SenderError Semantic Mapping

**DEFECT D-17 (LOW):** `terminate.rs` line 42 maps `CallResult::SenderError` → `TerminateError::NotFound`. Semantically, `SenderError` means the actor's mailbox is full or the actor is dying — it does NOT mean the instance "doesn't exist." This was likely inherited from Round 2 without reconsideration. The correct mapping would be either `TerminateError::Failed("actor mailbox error")` or a new `ActorDied` variant. The signal handler mock in signal_handler_test.rs uses the same pattern (`reply.send(Err(...))`) but the status handler in `status.rs` correctly maps `SenderError` → `ActorDied`.

Inconsistency: `status.rs` line 20 maps `SenderError` → `GetStatusError::ActorDied`, but `terminate.rs` line 42 maps `SenderError` → `TerminateError::NotFound`. **INCONSISTENT semantic mapping.**

---

## PHASE 5: The Bitter Truth

### YAGNI Check

No abstract traits with single implementers. No generic handlers for "future use." The code is minimal and purpose-built. **PASS.**

### The Sniff Test

The code reads like boring, obvious Rust. `call_cancel` is a straightforward `match` on `CallResult`. `map_terminate_result` is a straightforward `match` on `TerminateError`. The mock in tests is simple and clear. No cleverness detected. **PASS.**

### Inconsistency in INSTANCE_CALL_TIMEOUT

**DEFECT D-18 (MEDIUM):** `status.rs` line 7 still uses `Duration::from_millis(500)` for `INSTANCE_CALL_TIMEOUT`, while `terminate.rs` line 8 uses `Duration::from_secs(5)`. The D-10 fix was applied to terminate only. Status queries to instance actors still have a 500ms timeout, which is inconsistent and likely too short for the same class of operations. If the rationale for 5s applies to cancel calls, it applies equally to status calls on busy instances.

### Test Gap: No `terminate_failed` Test

The tests cover `NotFound`, `Timeout`, success, and bad path — but there is **no test for `TerminateError::Failed`**. This variant is mapped at `workflow.rs` line 151 to 500 INTERNAL_SERVER_ERROR. The mock actor doesn't exercise this path. Minor, but it's an untested error branch.

**DEFECT D-19 (LOW):** Missing test coverage for `TerminateError::Failed` → 500 path.

---

## Defect Summary

| ID   | Severity | Description | File:Line |
|------|----------|-------------|-----------|
| D-16 | **MEDIUM-HIGH** | `handle_cancel` discards `store.publish()` result — "no lost transitions" invariant violated on event store failure | `handlers.rs:124-129` |
| D-17 | LOW | `SenderError` → `NotFound` is semantically wrong; inconsistent with status.rs mapping to `ActorDied` | `terminate.rs:42` |
| D-18 | MEDIUM | `INSTANCE_CALL_TIMEOUT` in `status.rs` still 500ms — inconsistent with terminate.rs 5s fix | `status.rs:7` |
| D-19 | LOW | No test for `TerminateError::Failed` → 500 path | `terminate_handler_test.rs` |

---

## Round 2 Remediations — Final Verdict

The three Round 2 defects (D-01, D-06, D-10) are **correctly fixed and verified**. The code compiles, tests pass, and the stated fixes are in place.

However, **D-16 is a critical invariant violation** that was partially addressed (the event publish was added) but done in a way that silently drops failures. The entire point of the wtf-engine is durable execution with "guaranteed no lost transitions." Publishing a cancellation event and ignoring the result breaks this guarantee. This MUST be addressed before this bead can ship.

---

## VERDICT

**STATUS: REJECTED**

**Reason:** D-16 (silent event store publish failure in `handle_cancel`) violates the core invariant of the system. The fix from Round 1 added the publish call but did not ensure it succeeds. This is the kind of bug that causes silent data loss in production — exactly what the wtf-engine is designed to prevent.

**Required before Round 4:**
1. **D-16:** `handle_cancel` must propagate `store.publish()` failure. Options: (a) return `Err` from `handle_cancel` so the caller (orchestrator) knows cancel failed, (b) retry the publish, or (c) at minimum log at `error!` level instead of `let _ =`.
2. **D-18:** Align `INSTANCE_CALL_TIMEOUT` in `status.rs` with the 5s value in `terminate.rs` (or extract to a shared constant).
3. **D-17:** Consider mapping `SenderError` → a new `TerminateError::ActorDied` variant or `TerminateError::Failed(...)` for semantic correctness.

**D-19 is advisory** — nice to have but not blocking.
