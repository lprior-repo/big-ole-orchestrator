# Black Hat Review — Bead wtf-3hz: Implement terminate_workflow handler

**Reviewer:** Black Hat (glm-5-turbo)
**Date:** 2026-03-23
**Verdict:** REJECTED

---

## PHASE 1: Contract & Bead Parity

### 1.1 ADR-012 Compliance

| Requirement (ADR-012) | Status | Location |
|---|---|---|
| `DELETE /api/v1/workflows/:id` route | ✅ PASS | `routes.rs:22`, `app.rs:56` |
| Extract ActorRef from Extension | ✅ PASS | `workflow.rs:42` |
| Call `OrchestratorMsg::Terminate` with invocation_id from Path | ✅ PASS | `workflow.rs:49-51` |
| Return **204 No Content** on success | ✅ PASS | `workflow.rs:134` |
| Return **404** if workflow not found | ✅ PASS | `workflow.rs:135` |

**ADR-012 says nothing about terminate semantics** — the ADR's pseudocode (lines 186-231) only shows `start_workflow`, `get_workflow`, and `send_signal`. The terminate handler is specified only by the route table at line 37. The bead description correctly fills this gap. **Contract parity: PASS.**

### 1.2 Bead Spec Parity

Bead requires: "Return 204 No Content on success" — ✅
Bead requires: "Return 404 if workflow not found" — ✅
Bead requires: "Extract ActorRef from Extension" — ✅
Bead requires: "Call OrchestratorMsg::Terminate with invocation_id from Path" — ✅

**PASS — no contract violations.**

---

## PHASE 2: Farley Engineering Rigor

### 2.1 Function Length Constraint (>25 lines)

| Function | Lines | Verdict |
|---|---|---|
| `terminate_workflow` | lines 41-53 (13 lines) | ✅ PASS |
| `map_terminate_result` | lines 132-138 (7 lines) | ✅ PASS |
| `handle_terminate` | lines 10-23 (14 lines) | ✅ PASS |
| `call_cancel` | lines 25-42 (18 lines) | ✅ PASS |
| `handle_cancel` (instance) | lines 108-120 (13 lines) | ✅ PASS |

All functions under 25 lines. **PASS.**

### 2.2 Parameter Count (>5 params)

No function exceeds 5 parameters. **PASS.**

### 2.3 Functional Core / Imperative Shell Separation

**✅ GOOD:** The mapping functions (`map_terminate_result`, `map_actor_error`) are pure functions that take a `Result` and return a `Response`. No I/O hidden inside calculations.

### 2.4 Test Quality — BEHAVIOR vs IMPLEMENTATION ⚠️

**CRITICAL DEFECT D-01: ZERO integration tests for the terminate endpoint.**

The test landscape for `wtf-api` has:
- `tests/journal_test.rs` — 7 integration tests for journal
- `tests/validate_workflow_test.rs` — 5 integration tests for validate
- `tests/unit/signal_handler_test.rs` — 3 unit tests for signal
- **ZERO tests for terminate_workflow.** Not a single `DELETE /api/v1/workflows/:id` test exists anywhere in `wtf-api/tests/`.

The only test that exists is in `terminate.rs:52-69` — a **unit test** for `handle_terminate` at the actor level that only tests the `NotFound` path. There are NO tests for:
- ✗ Happy path: DELETE returns 204 for an existing workflow
- ✗ 404 path: DELETE returns 404 for a missing workflow
- ✗ Bad request: DELETE returns 400 for a malformed ID (no namespace slash)
- ✗ Actor timeout: DELETE returns 503 when the actor times out
- ✗ Actor dead: DELETE behavior when the orchestrator is unreachable

**Verdict: FAIL.** The signal handler has 3 integration tests in `tests/unit/signal_handler_test.rs`. The terminate handler has nothing at the HTTP layer. This is unacceptable for an endpoint that **destroys running workflows**.

---

## PHASE 3: NASA-Level Functional Rust (The Big 6)

### 3.1 Illegal States Unrepresentable

**PASS.** `TerminateError` is an enum — callers cannot construct invalid error states.

### 3.2 Parse, Don't Validate

**PASS.** The path ID is parsed at the boundary via `split_path_id` (line 45), which returns `Option<(String, InstanceId)>`. Invalid IDs are rejected at the HTTP boundary, never reaching the actor layer. `InstanceId` is a newtype wrapping a `String` — the domain primitive is trusted once parsed.

### 3.3 Types as Documentation

**DEFECT D-02: `reason` parameter is an untyped `String` — line 50.**

```rust
reason: "api-terminate".to_owned(),
```

The `reason` field flows through `InstanceMsg::Cancel { reason: String, ... }` (instance.rs:57) and is eventually just logged (handlers.rs:113-117). While this is a trace-level concern and doesn't warrant rejection, it's worth noting: the bead doesn't specify a `reason` field in the contract, yet the handler hardcodes `"api-terminate"`. If a user calls DELETE, the audit trail says `"api-terminate"` — there's no way to pass a custom reason. This is a **missing feature**, not a bug. Low severity.

### 3.4 Workflows as Explicit State Transitions

**PASS.** Terminate flows through explicit message passing:
```
HTTP → OrchestratorMsg::Terminate → handle_terminate → InstanceMsg::Cancel → handle_cancel
```
Each transition is a typed message. No hidden state mutation.

### 3.5 Newtypes

**PASS.** `InstanceId` is a newtype. No bare strings in domain models.

---

## PHASE 4: Ruthless Simplicity & DDD (Scott Wlaschin)

### 4.1 Option-Based State Machines

**PASS.** No `Option` used as a state machine. `OrchestratorState::get` returns `Option<&ActorRef<InstanceMsg>>` which is correctly handled.

### 4.2 The Panic Vector

**PASS — no `unwrap()`, `expect()`, or `panic!()` found in the terminate call chain.**

Verified:
- `workflow.rs:41-53` — no unwrap
- `workflow.rs:132-138` — match all arms
- `terminate.rs:10-23` — `let _ = reply.send(...)` correctly ignores send failure
- `terminate.rs:25-42` — `call_cancel` matches all `CallResult` variants
- `handlers.rs:108-120` — `let _ = reply.send(Ok(()))` correctly ignores send failure
- `mod.rs:82-83` — handler called via `.await`, no unwrap

**PASS — clean.**

### 4.3 Unnecessary `let mut`

**PASS.** No unnecessary mutability detected.

---

## PHASE 5: The Bitter Truth (Velocity & Legibility)

### 5.1 Cleverness Detection

**PASS.** The code is boring, linear, and obvious. No cleverness detected.

### 5.2 YAGNI Violations

**PASS.** No code built for "future use." Everything in the terminate chain is used today.

### 5.3 The Sniff Test

**DEFECT D-03 (CRITICAL): `handle_cancel` is a no-op masquerading as a handler.**

`handlers.rs:108-120`:
```rust
async fn handle_cancel(
    state: &InstanceState,
    reason: String,
    reply: RpcReplyPort<Result<(), WtfError>>,
) -> Result<(), ActorProcessingErr> {
    tracing::info!(...);
    let _ = reply.send(Ok(()));  // ← Immediately returns success
    Ok(())
}
```

**This function logs "cancellation requested" and immediately returns `Ok(())` without actually stopping the actor.** It doesn't:
- Set a cancellation flag on `InstanceState`
- Stop the actor
- Signal any in-flight work to abort
- Tell the orchestrator to deregister the instance

The workflow actor keeps running after `handle_cancel` returns. The HTTP client receives a `204 No Content`, believing the workflow is terminated, but the actor is still alive, processing events, dispatching activities, writing heartbeats.

This is **a lie told to the caller**. The DELETE endpoint says "workflow terminated" but the workflow is not terminated.

### 5.4 Race Condition: Actor Already Dead

**DEFECT D-04: Stale ActorRef in OrchestratorState.**

`handle_terminate` (terminate.rs:16) does:
```rust
match state.get(&instance_id) {
    None => { reply.send(Err(TerminateError::NotFound(instance_id))); }
    Some(actor_ref) => { call_cancel(actor_ref, reason).await; }
}
```

The `OrchestratorState::active` HashMap holds `ActorRef<InstanceMsg>`. When a workflow actor crashes or completes naturally, `handle_child_termination` (mod.rs:98-111) is called to deregister it. But **`handle_child_termination` is async and may not have run yet** when a user calls DELETE.

If the actor died between the orchestrator's check and `call_cancel`, `call_cancel` will hit:
- `Ok(CallResult::SenderError)` → mapped to `TerminateError::Failed("actor dropped reply")` → HTTP **500**

The user asked to delete something that no longer exists, and gets a 500 instead of a 404. This is **incorrect behavior**. The HTTP handler should return 204 (idempotent — "it's already gone") or 404, never 500.

### 5.5 Double Route Registration

**DEFECT D-05 (LOW): Duplicate route setup in `routes.rs` and `app.rs`.**

Both `routes.rs:22` and `app.rs:56` register `DELETE /workflows/:id` → `terminate_workflow`. If both `create_routes()` and `build_app()` are ever used together, this creates confusing duplicate route registration. One of these files appears to be dead code or a migration artifact.

---

## Defect Summary

| ID | Severity | Phase | Description |
|---|---|---|---|
| D-01 | **CRITICAL** | Farley | **ZERO HTTP-layer integration tests for terminate endpoint**. No test validates DELETE returns 204, 404, 400, or 503. An endpoint that destroys running workflows has NO test coverage at the HTTP boundary. |
| D-03 | **CRITICAL** | Bitter Truth | **`handle_cancel` is a no-op.** Returns `Ok(())` immediately without stopping the actor, setting a cancellation flag, or signalling in-flight work. The HTTP client is lied to — 204 is returned but the workflow keeps running. |
| D-04 | **HIGH** | Bitter Truth | **Race condition: dead actor returns 500 instead of 404/204.** If the actor dies between `state.get()` and `call_cancel`, the client gets a 500 `TerminateError::Failed("actor dropped reply")` instead of the semantically correct response. |
| D-05 | **LOW** | Bitter Truth | **Duplicate route registration** in `routes.rs` and `app.rs`. |
| D-02 | **LOW** | Functional Rust | **Hardcoded `reason` string** — no way for callers to specify a termination reason. Minor. |

---

## Mandated Actions (Before Re-review)

1. **[D-01]** Write at minimum 4 integration tests for the terminate endpoint:
   - Happy path: 204 on successful terminate
   - Not found: 404 for unknown instance
   - Bad request: 400 for malformed path ID
   - Actor timeout: 503 when orchestrator times out

2. **[D-03]** Implement actual cancellation in `handle_cancel`:
   - Set a `cancelled: bool` flag on `InstanceState`
   - Propagate cancellation to in-flight activity calls
   - Stop the actor after cancellation completes
   - Or, if immediate shutdown is intended, call `myself_ref.stop()` (or equivalent)

3. **[D-04]** Handle the dead-actor race in `call_cancel`:
   - Map `CallResult::SenderError` to `TerminateError::NotFound` (not `Failed`)
   - Or better: check actor liveness before calling, and map accordingly

---

## Final Verdict

**STATUS: REJECTED**

The terminate handler has **two critical defects**. D-03 (cancel is a no-op) is a **semantic lie** — the API promises termination but delivers nothing. D-01 (zero tests) means this lie has no test to catch it. Combined with D-04 (race condition), this endpoint is dangerous in production.

The HTTP boundary code itself (workflow.rs) is well-structured — clean separation, no unwrap, correct status codes. The problems are deeper: the actor layer doesn't actually terminate, and there are no tests to prove otherwise.

**Rewrite mandated. Fix D-03 and D-04 in the actor layer. Add integration tests (D-01). Come back for re-review.**
