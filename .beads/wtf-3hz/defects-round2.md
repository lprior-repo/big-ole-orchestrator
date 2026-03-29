# Black Hat Review — Bead vo-3hz: ROUND 2

**Reviewer:** Black Hat (glm-5-turbo)
**Date:** 2026-03-23
**Verdict:** REJECTED

---

## Round 1 Defect Verification

### D-03 (CRITICAL): handle_cancel was a no-op

**Status: FIXED** ✅

`handlers.rs:108-133` now:
1. Publishes `WorkflowEvent::InstanceCancelled { reason }` to the event store (line 120-128)
2. Sends `Ok(())` reply (line 130)
3. Calls `myself_ref.stop(Some(reason))` (line 131)

The pattern exactly matches `handle_completed`/`handle_failed` in `procedural_utils.rs:64-99`. The `myself_ref` parameter was correctly threaded through `handle_msg` (line 21) and `handle_procedural_msg` (line 30). **Verified fix.**

### D-04 (HIGH): SenderError mapped to Failed → now NotFound(404)

**Status: FIXED** ✅

`terminate.rs:42` now maps `Ok(CallResult::SenderError)` → `TerminateError::NotFound(instance_id.clone())`.
`terminate.rs:43` now maps `Err(_)` (mailbox failure) → `TerminateError::NotFound(instance_id.clone())`.

HTTP boundary at `workflow.rs:145` maps `TerminateError::NotFound` → `404 NOT_FOUND`. **Verified fix.**

### D-01 (CRITICAL): Zero HTTP integration tests

**Status: NOT FIXED** ❌

Grep of all files under `crates/vo-api/tests/` for "terminate" returns **zero results**. There is still not a single HTTP-layer test for `DELETE /api/v1/workflows/:id`.

The signal handler at `tests/unit/signal_handler_test.rs` demonstrates the exact test pattern required: a `MockOrchestrator` actor, `Router::new()` with `.layer(Extension(actor))`, and `oneshot` requests testing 200/400/404. The terminate handler has **none of this**.

The only test is the existing `terminate.rs:55-73` unit test for the `NotFound` path at the orchestrator level — which now asserts the correct variant after the D-04 fix. Good. But that's **one unit test**, not the **four HTTP integration tests** mandated in Round 1.

**Round 1 mandated:**
1. Happy path: DELETE returns 204 for existing workflow
2. Not found: DELETE returns 404 for unknown instance
3. Bad request: DELETE returns 400 for malformed path ID
4. Actor timeout: DELETE returns 503 when orchestrator times out

**Zero of these were added.** The author ignored the most critical mandate. D-01 remains open.

---

## Phase 1: Contract & Bead Parity

### 1.1 ADR-012 Compliance

| Requirement | Status | Evidence |
|---|---|---|
| `DELETE /api/v1/workflows/:id` → 204 on success | ✅ PASS | `workflow.rs:144`: `StatusCode::NO_CONTENT` |
| `DELETE /api/v1/workflows/:id` → 404 if not found | ✅ PASS | `workflow.rs:145`: `StatusCode::NOT_FOUND` |
| Extract ActorRef from Extension | ✅ PASS | `workflow.rs:42` |
| Call `OrchestratorMsg::Terminate` with instance_id | ✅ PASS | `workflow.rs:49-51` |

### 1.2 Timeout Response Code

**DEFECT D-06 (HIGH): Timeout on cancel returns 500 instead of 503.**

`terminate.rs:41`: `Ok(CallResult::Timeout)` → `TerminateError::Failed("cancel timed out".into())`.

This flows to `workflow.rs:146`: `_ => map_actor_error(res)` → but wait — this `Failed` variant is NOT `SenderError` or `MessagingErr`. Let me trace more carefully.

`call_cancel` returns `Err(TerminateError::Failed(...))` which is wrapped as `Ok(CallResult::Success(Err(TerminateError::Failed(...))))`. At `workflow.rs:143`:

```rust
Ok(CallResult::Success(Ok(()))) => StatusCode::NO_CONTENT,
Ok(CallResult::Success(Err(TerminateError::NotFound(id)))) => 404,
_ => map_actor_error(res)  // ← everything else, including TerminateError::Failed
```

The `_` catch-all maps `Ok(CallResult::Success(Err(TerminateError::Failed("cancel timed out"))))` to `map_actor_error`, which returns `500 INTERNAL_SERVER_ERROR` with code `"actor_error"`.

**This is wrong.** A timeout on the instance actor is a transient infrastructure condition — the instance might still be alive and processing. The correct HTTP code is **503 Service Unavailable** (like `GetStatus` does at `workflow.rs:136`). The caller should be able to retry. Instead, they get a 500 and assume the server is broken.

---

## Phase 2: Farley Engineering Rigor

### 2.1 Function Length (>25 lines)

| Function | Lines | Verdict |
|---|---|---|
| `handle_cancel` | lines 108-133 (25 lines) | ✅ BARELY PASS |
| `call_cancel` | lines 25-45 (20 lines) | ✅ PASS |
| `handle_terminate` | lines 10-23 (14 lines) | ✅ PASS |
| `map_terminate_result` | lines 142-148 (7 lines) | ✅ PASS |

### 2.2 Parameter Count (>5 params)

`handle_cancel` takes 4 parameters. `call_cancel` takes 3. All under 5. ✅ PASS.

### 2.3 Test Quality — CRITICAL GAP

**DEFECT D-01 (CARRIED FORWARD): Still zero HTTP integration tests.**

The signal handler (`signal_handler_test.rs`) proves the team knows how to write these tests. The journal handler has 7 integration tests. The validate handler has 5. Terminate has **zero**. This is a destructive endpoint that kills running workflows. No test proves it works.

### 2.4 Unit Test Correctness After D-04 Fix

The existing unit test at `terminate.rs:55-73` asserts:
```rust
assert!(matches!(reply, Err(TerminateError::NotFound(id)) if id == instance_id));
```

This tests the "instance not in registry" path. It does NOT test the "actor dead mid-call" path (SenderError → NotFound). That path has no unit test. **Minor but worth noting.**

---

## Phase 3: NASA-Level Functional Rust (The Big 6)

### 3.1 Illegal States Unrepresentable

**PASS.** `TerminateError` is a sum type. Cannot construct invalid error states.

### 3.2 Parse, Don't Validate

**PASS.** `split_path_id` at the boundary. `InstanceId` newtype trusted after parsing.

### 3.3 Types as Documentation

**Carried forward D-02 (LOW):** `reason: "api-terminate".to_owned()` at `workflow.rs:50` is an untyped string. Not blocking.

### 3.4 Workflows as Explicit State Transitions

**PASS.** Cancel flow: HTTP → `OrchestratorMsg::Terminate` → `handle_terminate` → `InstanceMsg::Cancel` → `handle_cancel` → publish event → `stop()`. Clean message chain.

### 3.5 Race Between stop() and the Reply

**DEFECT D-07 (MEDIUM): Potential race between `reply.send()` and `myself_ref.stop()`.**

`handlers.rs:130-131`:
```rust
let _ = reply.send(Ok(()));
myself_ref.stop(Some(reason));
```

In ractor, `stop()` triggers `post_stop` on the actor's event loop. The reply is sent via a oneshot channel to the caller. These two operations are sequential in the same async function, so **the reply will always be sent before stop() is called**. However, there's a subtlety: after `stop()` is called, the orchestrator's `handle_supervisor_evt` ( ActorTerminated ) runs asynchronously. Between `stop()` and the supervisor event, the orchestrator's `OrchestratorState` still has the actor registered. If a second DELETE request arrives in that window, `state.get()` will find the actor ref, `call_cancel` will hit `SenderError`, and now correctly returns 404 (thanks to D-04 fix). **This race is handled correctly post-fix.** ✅

### 3.6 Does stop() Guarantee the Supervisor Event Fires?

**DEFECT D-08 (MEDIUM): Event store publish failure is silently swallowed.**

`handlers.rs:121-128`:
```rust
if let Some(store) = &state.args.event_store {
    let _ = store.publish(&state.args.namespace, &state.args.instance_id, event).await;
}
```

The publish result is discarded with `let _ =`. If the event store is down, the `InstanceCancelled` event is **not persisted**, but the actor stops anyway. On recovery, the workflow will have no record of being cancelled. The event log will end at the last successful event, and a heartbeat-driven recovery could **restart the workflow from its last state** — exactly as if cancellation never happened.

The `handle_completed` and `handle_failed` functions in `procedural_utils.rs:72-78` and `91-97` have the exact same pattern — they also silently drop publish failures. So this is **consistent** with existing code. But for cancellation, the consequence is worse: a user explicitly asked to kill the workflow, the API said "204 — done", but the workflow silently resurrects on the next node.

This is a pre-existing architectural issue, not introduced by this bead. **Documented, not blocking.**

---

## Phase 4: Ruthless Simplicity & DDD (Scott Wlaschin)

### 4.1 Cancel vs Terminate Distinction

The code has a clear distinction:
- **Terminate** (HTTP layer + orchestrator): "Delete this workflow" — the user-facing operation
- **Cancel** (instance actor): "Stop processing" — the actor-level operation

This is correct. `TerminateError` is the orchestrator-level error (NotFound/Failed). The instance reply is `Result<(), VoError>`. The type boundaries are clean. ✅ PASS

### 4.2 TerminateError Expressiveness

**DEFECT D-09 (LOW): `TerminateError::Failed(String)` is a String-typed error.**

`errors.rs:23`: `Failed(String)` is an opaque bag. The caller gets a string like `"cancel timed out"` or a `VoError` message. There's no way to programmatically distinguish between "timed out", "event store down", or "actor panicked" at the HTTP boundary.

This should ideally be:
```rust
enum TerminateError {
    NotFound(InstanceId),
    Timeout(InstanceId),
    CancelFailed { id: InstanceId, reason: String },
}
```

The HTTP handler could then map `Timeout` → 503 and `CancelFailed` → 500. This would fix D-06 at the type level.

### 4.3 Panic Vector

**PASS.** No `unwrap()`, `expect()`, `panic!()` in the terminate call chain. All error paths use `let _ = reply.send(...)` or `match`.

### 4.4 No Option-Based State Machines

**PASS.**

---

## Phase 5: The Bitter Truth (Velocity & Legibility)

### 5.1 What if the instance is mid-workflow when stop() is called?

`actor.rs:75-77` (`post_stop`):
```rust
if let Some(handle) = state.procedural_task.take() {
    handle.abort();
}
```

`stop()` triggers `post_stop`, which aborts the procedural task handle. For FSM/DAG paradigms, the actor just stops processing messages. This is correct behavior — `stop()` is the ractor way to terminate. No issue here. ✅

### 5.2 The InstanceCancelled Event and Event Store Down

Covered in D-08 above. The event is silently lost. Workflow may resurrect on recovery. **Pre-existing issue, not introduced by this bead.**

### 5.3 Double Route Registration — D-05 from Round 1

`app.rs:56` and `routes.rs:22` both register `DELETE /workflows/:id`.

**Status: UNRESOLVED.** `create_routes()` is never called from production code (grep shows zero callers outside `routes.rs` itself). It's dead code. `build_app()` in `app.rs` is the actual production router.

This is a maintenance hazard — someone will eventually use `create_routes()` and get confused by the duplicate. **Low severity, still open.**

### 5.4 INSTANCE_CALL_TIMEOUT Value

`terminate.rs:8`: `const INSTANCE_CALL_TIMEOUT: Duration = Duration::from_millis(500);`

500ms is **aggressive** for an operation that publishes to the event store and then stops the actor. If the event store publish takes >500ms (NATS hiccup), the cancel will time out and return `TerminateError::Failed("cancel timed out")` → HTTP 500 (per D-06).

The orchestrator-level `ACTOR_CALL_TIMEOUT` is 5 seconds (`handlers/mod.rs:23`). The instance-level cancel timeout is 10x shorter. This seems backwards — the instance-level operation (I/O to event store + actor shutdown) is more likely to be slow than the orchestrator-level lookup.

**DEFECT D-10 (MEDIUM): INSTANCE_CALL_TIMEOUT of 500ms is too aggressive for a cancel operation that does I/O.**

### 5.5 The "publish-then-stop" Ordering

`handlers.rs:130-131`:
```rust
let _ = reply.send(Ok(()));
myself_ref.stop(Some(reason));
```

The reply is sent BEFORE `stop()`. This means the HTTP client gets 204 BEFORE the actor actually stops. There's a window where the client thinks "workflow terminated" but the actor is still running. This is an **eventual consistency model**, which is acceptable for this system — but worth documenting.

Compare with `handle_completed`/`handle_failed` in `procedural_utils.rs:72-80`: same pattern — publish event, stop, NO reply at all (those are fire-and-forget signals from the procedural workflow, not RPC calls). So `handle_cancel` is actually **more responsive** than the completion/failure paths. ✅

---

## Defect Summary

| ID | Severity | Phase | Status | Description |
|---|---|---|---|---|
| D-01 | **CRITICAL** | Farley | **NOT FIXED** | **Zero HTTP integration tests for terminate.** Signal handler has 3 tests proving the pattern exists. Author ignored Round 1 mandate to add 4 tests (204/404/400/503). |
| D-06 | **HIGH** | Contract | **NEW** | **Cancel timeout returns 500 instead of 503.** `CallResult::Timeout` in `call_cancel` maps to `TerminateError::Failed` which falls through to the `_` catch-all in `map_terminate_result`, returning HTTP 500. Should be 503 (retryable). |
| D-09 | **LOW** | DDD | **NEW** | `TerminateError::Failed(String)` is opaque — cannot programmatically distinguish timeout from other failures at HTTP layer. Would fix D-06 at the type level. |
| D-05 | **LOW** | Bitter Truth | **CARRIED** | Duplicate route registration in `routes.rs` (dead code) and `app.rs` (production). |
| D-02 | **LOW** | Functional Rust | **CARRIED** | Hardcoded `reason: "api-terminate"` — no custom reason from caller. |
| D-08 | **MEDIUM** | Functional Rust | **DOCUMENTED** | Event store publish failure silently dropped — `InstanceCancelled` event may not persist. Pre-existing pattern, not introduced by this bead. Workflow may resurrect on recovery. |
| D-10 | **MEDIUM** | Bitter Truth | **NEW** | `INSTANCE_CALL_TIMEOUT` is 500ms — 10x shorter than orchestrator timeout — for an operation that does I/O to event store. Likely to cause spurious timeouts under load. |

### Verified Fixed (Round 1)

| ID | Severity | Status | Notes |
|---|---|---|---|
| D-03 | CRITICAL | ✅ FIXED | `handle_cancel` now publishes `InstanceCancelled` event and calls `myself_ref.stop()`. Matches `handle_completed`/`handle_failed` pattern. |
| D-04 | HIGH | ✅ FIXED | `SenderError` and send failure now map to `TerminateError::NotFound` → HTTP 404. |

---

## Mandated Actions (Before Round 3)

1. **[D-01] WRITE THE DAMN TESTS.** Four HTTP integration tests following the exact pattern in `signal_handler_test.rs`:
   - 204 on successful cancel (mock orchestrator replies Ok)
   - 404 for unknown instance (mock orchestrator replies NotFound)
   - 400 for malformed path ID (no namespace slash)
   - 503 on orchestrator timeout (mock doesn't reply)

2. **[D-06]** Either:
   - Add `TerminateError::Timeout(InstanceId)` variant, map in `call_cancel`, and handle in `map_terminate_result` → 503
   - OR: Handle `TerminateError::Failed` specifically in `map_terminate_result` and check if the message contains "timed out" (hacky but pragmatic)

3. **[D-10]** Increase `INSTANCE_CALL_TIMEOUT` to at least match `ACTOR_CALL_TIMEOUT` (5s), or add a comment explaining why 500ms is appropriate despite the event store I/O in the cancel path.

---

## Final Verdict

**STATUS: REJECTED**

Round 1's two critical defects (D-03 no-op cancel, D-04 dead-actor 500) are properly fixed. The actor layer now actually stops and publishes the cancellation event. The SenderError mapping to 404 is correct.

**But D-01 remains unaddressed.** The author fixed the code but completely ignored the testing mandate. For an endpoint that **destroys running workflows**, zero HTTP-layer tests is unacceptable. The signal handler proves the team has the pattern — they just didn't use it.

Additionally, the timeout-on-cancel returns 500 (D-06) instead of 503, which means a transient NATS hiccup will cause the client to believe the server is broken rather than just busy. The 500ms timeout (D-10) makes this worse.

Fix the tests. Fix the timeout mapping. Come back for Round 3.
