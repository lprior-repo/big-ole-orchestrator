# BLACK HAT REVIEW — Bead vo-72g: Implement get_workflow handler

**Reviewer:** Black Hat  
**Verdict:** **REJECTED**  
**Date:** 2026-03-23  

---

## PHASE 1: Contract & Bead Parity — FAIL

### DEFECT-01: Bead specifies `WorkflowStatus`; implementation returns `V3StatusResponse`

The bead description states verbatim:

> "Return WorkflowStatus or 404 if not found"

The bead's code skeleton in `beads/route-defs.json` line 589:

```rust
pub async fn get_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(invocation_id): Path<String>,
) -> Result<Json<WorkflowStatus>, StatusCode> {
```

The ADR-012 reference at line 206 also says `Result<Json<WorkflowStatus>, StatusCode>`.

**What actually exists** at `workflow.rs:126`:

```rust
Ok(CallResult::Success(Some(s))) => (StatusCode::OK, Json(V3StatusResponse::from(s))).into_response(),
```

The handler returns `V3StatusResponse`, not `WorkflowStatus`. These are **structurally identical** types (responses.rs:39–46 vs responses.rs:130–137 — same 6 fields, same types), but they are **distinct, independent structs**. This means the bead contract is not satisfied as written, and worse, it creates type confusion (see DEFECT-04).

**Severity: HIGH** — The bead contract is violated. The types are duplicated, not unified.

### DEFECT-02: Bead specifies `call_t!` macro — it does not exist

The bead description says:

> "Use call_t! with timeout"

No `call_t!` macro exists anywhere in this codebase. The implementation uses raw `master.call(...)` directly. The bead specification is fiction. Either the macro should have been implemented, or the bead description is a hallucination from the planning phase.

**Severity: MEDIUM** — Bead specification contains a non-existent construct, making the contract unreliable.

### DEFECT-03: Bead says `Path(invocation_id)` — implementation uses `Path(id)` and splits `namespace/instance_id`

The bead and ADR-012 both reference `invocation_id` as a single path parameter. The actual implementation at `workflow.rs:30-35` expects a compound `namespace/instance_id` format and calls `split_path_id()`:

```rust
Path(id): Path<String>,
// ...
let (_, inst_id) = match split_path_id(&id) {
```

The `split_path_id` call discards the namespace (`_`), meaning the namespace component of the URL path is parsed then thrown away. The handler does not verify the namespace matches the instance — a **silent data integrity bug**.

**Severity: HIGH** — Namespace is parsed from the URL but completely ignored. An instance `payments/ABC` can be queried via `completely-different-ns/ABC` and it will succeed. This is a routing/authorization correctness defect.

---

## PHASE 2: Farley Engineering Rigor — FAIL

### DEFECT-04: Zero HTTP-layer tests for `get_workflow`

There are **no tests** for `get_workflow`. Period.

- `workflow.rs:187-249` contains tests only for `split_path_id`, `parse_paradigm`, and `paradigm_to_str` — all pure utility functions.
- No `#[tokio::test]` exercises `get_workflow`, `map_status_result`, or the 404/200/503 code paths.
- No integration test exists in `crates/vo-api/tests/` (the directory is empty — no `*.rs` files found).
- The `signal_handler_test.rs` proves the pattern exists (spawn mock actor, build app, oneshot request) but was not replicated for workflow handlers.

The `list_workflows` handler at line 59-63 has the same problem — untested.

The `app.rs:104-105` comment admits this:

```rust
// We can't easily create a real ActorRef in tests without a running ractor system.
```

That's a failure of test infrastructure, not an excuse. The signal handler test already solved this with a `MockOrchestrator`. The author was lazy and didn't replicate the pattern.

**Severity: CRITICAL** — Zero test coverage on a production HTTP handler. This is a fireable offense.

### DEFECT-05: `map_actor_error` swallows all error context

`workflow.rs:140-145`:

```rust
fn map_actor_error<T>(res: Result<CallResult<T>, MessagingErr<OrchestratorMsg>>) -> impl IntoResponse {
    match res {
        Ok(CallResult::Timeout) => (StatusCode::SERVICE_UNAVAILABLE, Json(ApiError::new("actor_timeout", "timeout"))),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError::new("actor_error", "actor failed"))),
    }
}
```

The `_` catch-all collapses `CallResult::ActorRestarted`, `CallResult::SupervisorFailure`, `MessagingErr::ActorNotFound`, `MessagingErr::ChannelFull`, and `MessagingErr::ChannelClosed` into a single "actor failed" string. The caller has **zero diagnostic information** to distinguish between a dead actor, a full mailbox, and a network partition. In production, this is a debugging nightmare.

**Severity: HIGH** — Error information is deliberately destroyed.

---

## PHASE 3: NASA-Level Functional Rust (The Big 6) — PARTIAL PASS

### PASS: No `unwrap()`/`expect()` in handler code path

The handler itself (`workflow.rs:28-38`) and `map_status_result` (lines 124-130) use exhaustive `match` with no panicking. Good.

### PASS: Timeout is handled correctly (mechanically)

`ACTOR_CALL_TIMEOUT` (5s) is applied at the orchestrator level. `INSTANCE_CALL_TIMEOUT` (500ms) is applied at the instance actor level. Both produce `CallResult::Timeout` which maps to 503.

### DEFECT-06: Instance actor timeout silently returns 404 instead of 503

`status.rs:14-17` (the orchestrator's `handle_get_status`):

```rust
match actor_ref.call(InstanceMsg::GetStatus, Some(INSTANCE_CALL_TIMEOUT)).await {
    Ok(CallResult::Success(snapshot)) => Some(snapshot),
    _ => None,  // <--- ALL failures become None
}
```

If the instance actor exists in the registry but is **wedged** (e.g., stuck in a long procedural sleep, processing a massive event batch), the 500ms timeout fires. The orchestrator catches `CallResult::Timeout` and returns `None`. The handler at `workflow.rs:127` then maps `None` to:

```rust
Ok(CallResult::Success(None)) => (StatusCode::NOT_FOUND, Json(ApiError::new("not_found", id))).into_response(),
```

**A wedged, running instance returns 404 as if it doesn't exist.** This is semantically wrong. A timeout is NOT "not found." The client will retry with backoff, but if it uses cache-busting or creates a duplicate instance, you get orphaned workflows.

The correct behavior: propagate timeout information from `handle_get_status` so the handler can return 503, not 404.

**Severity: CRITICAL** — Semantic error in error handling causes silent data corruption under load.

### DEFECT-07: `InstanceId` is parsed from raw string without validation at the boundary

`mod.rs:50-54`:

```rust
pub(crate) fn split_path_id(path: &str) -> Option<(String, InstanceId)> {
    let slash = path.find('/')?;
    let (ns, id) = path.split_at(slash);
    Some((ns.to_owned(), InstanceId::new(id[1..].to_owned())))
}
```

`InstanceId::new()` accepts any string — no length check, no ULID format validation, no character whitelist. Compare to `validate_start_req` at line 103 which uses `InstanceId::try_new()` with proper validation. The get path skips validation entirely.

**Severity: MEDIUM** — Inconsistent boundary parsing. "Parse, Don't Validate" is violated on the read path.

---

## PHASE 4: Ruthless Simplicity & DDD (Scott Wlaschin) — FAIL

### DEFECT-08: `WorkflowStatus` and `V3StatusResponse` are identical structs — delete one

`responses.rs:39-46` (`WorkflowStatus`) and `responses.rs:130-137` (`V3StatusResponse`) have **the exact same fields**:

```
instance_id: String
namespace: String
workflow_type: String
paradigm: String
phase: String
events_applied: u64
```

Two structs, same shape, different names, different purposes — except they're used for the same purpose (instance status). `WorkflowStatus` is used by `ListWorkflowsResponse.workflows` (line 91), and `V3StatusResponse` is used by `get_workflow` and `list_workflows`. This means `list_workflows` uses `V3StatusResponse` while the type `ListWorkflowsResponse` references `WorkflowStatus` — they're inconsistent even within the same module.

This is textbook duplication. One type should exist. Pick one, delete the other, and use a type alias if versioning is the concern.

**Severity: HIGH** — Two identical types creates confusion, maintenance burden, and subtle bugs when one is updated and the other isn't.

### DEFECT-09: `ApiError` and `ErrorResponse` are also near-duplicates

`responses.rs:141-154` (`ApiError`) and `responses.rs:157-163` (`ErrorResponse`) have the same `error` + `message` fields. `ErrorResponse` adds `retry_after_seconds`. Both exist in the same file. The handler uses `ApiError`; `ErrorResponse` appears to be a newer type with retry semantics that was never adopted.

**Severity: MEDIUM** — Ongoing type sprawl in the responses module.

---

## PHASE 5: The Bitter Truth (Velocity & Legibility) — FAIL

### DEFECT-10: `do_replay_to` at 14 lines in a single `loop {}` with no structure

`workflow.rs:156-170` contains a `loop` with nested `match` on `stream.next_event()`. While it's under 25 lines, it uses `loop` without a clear loop invariant or max-iteration guard. If `next_event()` returns events infinitely (bug in the store), this handler never returns. A replay loop should have a `max_events` bound.

**Severity: LOW** — Defensive programming gap, but real under adversarial conditions.

### DEFECT-11: `load_snapshot` constructs a fake `InstanceArguments` to initialize empty state

`workflow.rs:179-184`:

```rust
Ok((vo_actor::instance::state::initialize_paradigm_state(&vo_actor::InstanceArguments {
    namespace: NamespaceId::new(""), instance_id: id.clone(), workflow_type: "".to_owned(), paradigm,
    input: Bytes::new(), engine_node_id: "".to_owned(), snapshot_db: None,
    procedural_workflow: None, workflow_definition: None,
    event_store: None, state_store: None, task_queue: None,
}), 1))
```

This builds a throwaway `InstanceArguments` with all empty/None fields just to get a default paradigm state. This is a factory function masquerading as a constructor. The `initialize_paradigm_state` function should accept a `WorkflowParadigm` alone, not a full `InstanceArguments`. The author is working around a bad API instead of fixing it.

**Severity: MEDIUM** — Leaking internal construction details into HTTP handler code.

### DEFECT-12: `_` discards namespace in `get_workflow` — already flagged but the pattern is repeated in `terminate_workflow`

`workflow.rs:45`:

```rust
let (_, inst_id) = match split_path_id(&id) {
```

Same silent namespace discard in `terminate_workflow`. If namespace-based isolation is ever enforced, every handler will need retrofitting.

**Severity: MEDIUM** — Systemic pattern of ignoring the namespace path component.

---

## SUMMARY TABLE

| # | Phase | Severity | Title |
|---|-------|----------|-------|
| 01 | 1 - Contract | **HIGH** | Returns `V3StatusResponse` not `WorkflowStatus` as specified |
| 02 | 1 - Contract | MEDIUM | `call_t!` macro specified but does not exist |
| 03 | 1 - Contract | **HIGH** | Namespace parsed then discarded — routing bug |
| 04 | 2 - Farley | **CRITICAL** | Zero HTTP-layer tests for `get_workflow` |
| 05 | 2 - Farley | **HIGH** | `map_actor_error` swallows all error context |
| 06 | 3 - Functional Rust | **CRITICAL** | Instance timeout returns 404 instead of 503 |
| 07 | 3 - Functional Rust | MEDIUM | No ID format validation on read path |
| 08 | 4 - DDD | **HIGH** | `WorkflowStatus` and `V3StatusResponse` are identical duplicates |
| 09 | 4 - DDD | MEDIUM | `ApiError` and `ErrorResponse` are near-duplicates |
| 10 | 5 - Bitter Truth | LOW | Replay loop has no max-iteration guard |
| 11 | 5 - Bitter Truth | MEDIUM | Fake `InstanceArguments` to get default state |
| 12 | 5 - Bitter Truth | MEDIUM | Namespace discard pattern repeated in `terminate_workflow` |

---

## REQUIRED REMEDIATION (Mandatory Before Re-review)

1. **CRITICAL-6**: Change `handle_get_status` to return `Result<Option<InstanceStatusSnapshot>, StatusError>` where `StatusError` distinguishes `Timeout` from `NotFound`. Propagate to handler so 404 and 503 are semantically correct.
2. **CRITICAL-4**: Write HTTP-level tests for `get_workflow` using the `MockOrchestrator` pattern from `signal_handler_test.rs`. Cover: 200 with valid snapshot, 404 for missing instance, 503 for timeout, 400 for malformed path.
3. **HIGH-1 + HIGH-8**: Delete `WorkflowStatus`. Make it a type alias for `V3StatusResponse`, or vice versa. Pick one name. Update `ListWorkflowsResponse` accordingly.
4. **HIGH-3 + HIGH-12**: Stop discarding the namespace. Either use it to scope the lookup, or change the URL format to not include it. Currently the API lies — the path implies namespace scoping but the handler ignores it.
5. **HIGH-5**: Log or include the actual error variant in `map_actor_error` so operators can diagnose failures.
6. **Update the bead description** to reflect reality (no `call_t!`, correct return type) or update the code to match the bead.

---

## VERDICT

**STATUS: REJECTED**

Two CRITICAL defects (silent 404 on timeout, zero test coverage), four HIGH defects (contract violation, type duplication, namespace routing bug, error context destruction), and a litany of MEDIUM/LOW issues. This code ships broken semantics under load and has no safety net. Rewrite the `handle_get_status` return type, add tests, unify the response types, and stop lying to the caller about what 404 means.
