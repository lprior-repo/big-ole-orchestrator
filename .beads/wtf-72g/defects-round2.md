# Black Hat Review — Round 2: Bead wtf-72g (get_workflow handler)

**Reviewer:** Black Hat Reviewer  
**Date:** 2026-03-23  
**Files Inspected:** 6/6  
**Status:** REJECTED

---

## Round 1 Defect Verification

### DEFECT-06 (CRITICAL): Timeout returns 503 not 404 — VERDICT: FIXED ✅

`GetStatusError::Timeout` now exists in `errors.rs:28-31`.  
`map_status_result` at `workflow.rs:135-137` correctly maps `GetStatusError::Timeout` to `503 SERVICE_UNAVAILABLE`.  
The actor handler in `status.rs:19` returns `Err(GetStatusError::Timeout)` on non-success call results.

**Scorecard: 3/3 paths verified. This defect is closed.**

---

### DEFECT-05 (HIGH): map_actor_error collapses all errors — VERDICT: PARTIALLY FIXED ⚠️

`map_actor_error` now distinguishes 5 distinct error variants (lines 150-177):
- `CallResult::Timeout` → 503
- `CallResult::SenderError` → 503
- `MessagingErr::ChannelClosed` → 503
- `MessagingErr::InvalidActorType` → 500
- `MessagingErr::SendErr` → 503

**Remaining problem:** Line 172 has a `_ =>` wildcard catch-all that returns 500 `"actor_error"`. This matches `Ok(CallResult::Success(_))` — meaning if the orchestrator successfully replies with a value but the handler somehow falls through, the client gets a generic 500 instead of the actual response. In practice this branch should be unreachable if `map_status_result` is called correctly, but the type system does not enforce this. The wildcard is a **code smell** that could mask future regressions.

---

### DEFECT-04 (CRITICAL): Zero HTTP tests for get_workflow — VERDICT: NOT FIXED ❌

**Zero HTTP integration tests exist for `get_workflow`.** Verified by:
- No file matching `*get_workflow*test*` or `*workflow*test*` exists under `crates/wtf-api/tests/`
- No grep hit for `get_workflow` or `GetStatus` in any test file
- The only tests in `workflow.rs` are unit tests for `split_path_id` and `parse_paradigm` (lines 219-282) — **these are parser utility tests, not handler tests**

A perfectly valid test scaffold exists in `signal_handler_test.rs` that demonstrates the pattern: mock `Actor`, `Router::new()`, `oneshot()`, assert status code, parse JSON body. **This pattern was not replicated for `get_workflow`.**

Missing test coverage for ALL handler paths:
| Path | Expected Status | Tested? |
|------|----------------|---------|
| Valid instance, status returned | 200 | ❌ |
| Instance not in registry | 404 | ❌ |
| Instance actor timeout | 503 | ❌ |
| Invalid path (no slash) | 400 | ❌ |
| Orchestrator actor timeout | 503 | ❌ |
| Orchestrator channel closed | 503 | ❌ |

**7 untested HTTP paths for a single handler. This is negligent.**

---

### DEFECT-01 (HIGH): Returns V3StatusResponse not WorkflowStatus — VERDICT: NOT FIXED ❌

`responses.rs` contains **three structurally identical status types**:

1. **`WorkflowStatus`** (lines 39-46): `{instance_id, namespace, workflow_type, paradigm, phase, events_applied}`
2. **`V3StatusResponse`** (lines 130-137): `{instance_id, namespace, workflow_type, paradigm, phase, events_applied}`
3. **`InstanceStatusSnapshot`** (instance.rs:89-96): `{instance_id: InstanceId, namespace: NamespaceId, workflow_type: String, paradigm: WorkflowParadigm, phase: InstancePhaseView, events_applied: u64}`

All three have the **exact same fields** with the only difference being that `InstanceStatusSnapshot` uses domain types (`InstanceId`, `NamespaceId`, `WorkflowParadigm`, `InstancePhaseView`) while `V3StatusResponse` and `WorkflowStatus` use raw `String`.

The handler at `workflow.rs:130` uses `V3StatusResponse::from(s)`. The `From` impl at lines 179-186 does a straightforward field-by-field mapping.

**`WorkflowStatus` is dead code.** It's only referenced in `ListWorkflowsResponse.workflows: Vec<WorkflowStatus>` (line 91), which itself is never used by any handler — `list_workflows` at line 61 uses `V3StatusResponse::from()` directly.

**Three types, zero types.** Kill `WorkflowStatus` and `ListWorkflowsResponse`, or kill `V3StatusResponse` and use `WorkflowStatus` consistently. Pick ONE.

---

## NEW FINDINGS — Round 2

### DEFECT-R2-01 (CRITICAL): 503 Timeout misleads clients into retrying dead workflows

`status.rs:17-19`:
```rust
match actor_ref.call(InstanceMsg::GetStatus, Some(INSTANCE_CALL_TIMEOUT)).await {
    Ok(CallResult::Success(snapshot)) => Ok(Some(snapshot)),
    _ => Err(GetStatusError::Timeout),
}
```

The `_` wildcard here catches **ALL** non-success call results, including:
- `CallResult::Timeout` — instance actor is slow/gone, retry is reasonable
- `CallResult::SenderError` — **the actor has PANICKED or been KILLED**

If the instance actor has been killed (e.g., crash recovery completed, instance reached terminal state, or OOM), `SenderError` fires, and the handler reports 503 "instance actor timed out." The client retries. The instance is **dead**. It will never come back. The client is now in a retry loop against a ghost.

**The error taxonomy lacks a `GetStatusError::ActorDied` variant.** `Timeout` and `ActorDied` are fundamentally different: one is transient, the other is permanent. A 503 for a permanently dead instance is a lie.

---

### DEFECT-R2-02 (HIGH): 503 responses lack Retry-After header

`responses.rs` already defines:
- `RetryAfterSeconds` newtype (newtypes.rs:142) — enforces > 0 at compile time
- `ErrorResponse` struct (responses.rs:158) — has `retry_after_seconds: Option<RetryAfterSeconds>`
- `ErrorResponse::new()` validates that retryable errors have `retry_after_seconds` and non-retryable don't

**None of these are used.** Every handler in `workflow.rs` returns `ApiError` (which has NO `retry_after` field). `ErrorResponse` is only referenced in a unit test at `types/tests.rs:105`.

The 503 responses at lines 136, 152, 157, 161, 169 all say "timed out" / "unavailable" but give the client **zero guidance on when to retry**. Per RFC 7231 §6.6.3, `Retry-After` is **SHOULD** for 503 responses.

**The infrastructure exists but is completely disconnected from the handlers.** Dead code pretending to be architecture.

---

### DEFECT-R2-03 (MEDIUM): `get_instance_paradigm` silently swallows GetStatusError

`workflow.rs:90-98`:
```rust
async fn get_instance_paradigm(...) -> Result<WorkflowParadigm, anyhow::Error> {
    let res = master.call(|tx| OrchestratorMsg::GetStatus { ... }, Some(ACTOR_CALL_TIMEOUT)).await;
    if let Ok(CallResult::Success(Ok(Some(snap)))) = res {
        return Ok(snap.paradigm);
    }
    // Falls through to sled lookup on ANY failure
    let metadata = store.get_instance_metadata(id).await?...
```

If the instance exists but is timing out, this silently falls through to the sled store lookup. If the sled store also fails, the error message is "instance metadata not found" — which tells the caller "doesn't exist" when the truth is "exists but timed out." The error semantics are **corrupted** by this fallback path.

This function is used by `replay_to` (line 76). If a client tries to replay a live but slow instance, they get 404 instead of 503. **The Round 1 fix to `map_status_result` is bypassed entirely by this code path.**

---

### DEFECT-R2-04 (MEDIUM): `list_workflows` doesn't use the fixed error handling

`workflow.rs:59-63`:
```rust
let res = master.call(|tx| OrchestratorMsg::ListActive { reply: tx }, Some(ACTOR_CALL_TIMEOUT)).await;
match res {
    Ok(CallResult::Success(snapshots)) => (StatusCode::OK, Json(snapshots.into_iter().map(V3StatusResponse::from).collect::<Vec<_>>())).into_response(),
    _ => map_actor_error(res).into_response(),
}
```

`ListActive` has `reply: RpcReplyPort<Vec<InstanceStatusSnapshot>>` — it returns the snapshots directly, **no `Result<_, GetStatusError>` wrapping**. This means if any instance is in a degraded state when the orchestrator collects the list, the entire list call fails and the client gets a blanket 503/500. No way to distinguish "one instance is slow" from "the whole orchestrator is down."

Not a regression, but a design inconsistency with the `GetStatus` error taxonomy that was supposedly fixed.

---

### DEFECT-R2-05 (LOW): `map_status_result` doesn't handle nested `CallResult::Success(Err(_))` for unknown error variants

`workflow.rs:135` only matches `GetStatusError::Timeout`. If `GetStatusError` gains new variants in the future (which it should, per R2-01), this match arm silently falls through to the wildcard at line 138 (`map_actor_error`), which returns 500 `"actor_error"` — hiding the actual domain error.

This should be either exhaustive or have a deliberate catch-all that maps unknown domain errors to 500 with the error message preserved.

---

## Phase Summary

| Phase | Verdict |
|-------|---------|
| **1. Contract Parity** | ⚠️ PARTIAL — Timeout→503 fixed. Missing tests. Type duplication unresolved. |
| **2. Farley Constraints** | ❌ FAIL — Zero HTTP tests. 7 untested code paths. Signal handler test exists as a proven scaffold but was not used. |
| **3. Functional Rust (Big 6)** | ❌ FAIL — `GetStatusError` is a 1-variant enum (the "enum with one variant" anti-pattern). Wildcard `_` in `status.rs:19` collapses `Timeout` + `SenderError` + `ActorStopped` into a single meaningless variant. Boolean disguised as enum. |
| **4. Strict DDD** | ❌ FAIL — Three identical status types. `WorkflowStatus` is dead code. `ErrorResponse` + `RetryAfterSeconds` infrastructure exists but is entirely disconnected from handlers. Parse-Don't-Validate violated at system boundary (raw strings for IDs, paradigm, phase). |
| **5. Bitter Truth** | ❌ FAIL — 503 misleads clients about dead workflows. `replay_to` bypasses the timeout fix entirely via silent fallback. No `Retry-After` headers. Dead code everywhere. Author added types and error variants to look busy but didn't write a single HTTP test. |

---

## Verdict: REJECTED

### Mandatory Actions Before Round 3

1. **[CRITICAL]** Write HTTP integration tests for `get_workflow` covering all 7 paths listed above. Use the `signal_handler_test.rs` scaffold. No excuses.

2. **[CRITICAL]** Expand `GetStatusError` to distinguish `Timeout` (transient) from `ActorDied` (permanent). The handler should return 503 for timeout and 410 Gone (or 404 with explanation) for dead actors.

3. **[HIGH]** Kill `WorkflowStatus` and `ListWorkflowsResponse`. They are dead code. One status response type or die trying.

4. **[HIGH]** Either wire `ErrorResponse` (with `Retry-After`) into the 503 paths, or delete `ErrorResponse` and `RetryAfterSeconds`. Pick one. Dead infrastructure is worse than no infrastructure.

5. **[MEDIUM]** Fix `get_instance_paradigm` to propagate `GetStatusError::Timeout` instead of silently falling through to sled. This function currently bypasses the entire Round 1 fix.

6. **[MEDIUM]** Replace the `_ =>` wildcard in `status.rs:19` with explicit error matching. A single-variant enum with a wildcard match is a boolean with extra steps.
