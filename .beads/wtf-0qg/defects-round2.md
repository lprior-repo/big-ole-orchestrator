# Black Hat Review — Round 2 — Bead vo-0qg (spawn_workflow / spawn_and_register)

**Reviewer:** Black Hat  
**Date:** 2026-03-23  
**Files inspected:** 6 files across `vo-actor/src/master/handlers/`, `messages/`, `master/state.rs`, `master/mod.rs`  
**Verdict:** `STATUS: REJECTED`

---

## Round 1 Defect Verification

| ID | Severity | Status | Verdict |
|----|----------|--------|---------|
| N-01 | CRITICAL | SUPPOSEDLY FIXED | **HALF-FIXED — still insufficient. See N-07 below** |
| N-02 | HIGH | SUPPOSEDLY FIXED | **NOT FIXED. Zero integration tests exist.** |
| N-04 | MEDIUM | SUPPOSEDLY FIXED | **FIXED.** `validate_request_rejects_when_instance_already_exists` at start.rs:153-166 covers `AlreadyExists`. ✓ |
| N-05 | MEDIUM | SUPPOSEDLY FIXED | **NOT FIXED.** Duplicate `InstanceArguments` construction still in two places. See N-06 below |

---

## PHASE 1: Contract & Bead Parity

**No bead spec found.** The `.beads/vo-0qg/` directory was empty before this review. There is no `contract-spec.md` or `martin-fowler-tests.md` to verify against. The bead contract is a **fiction** — there's nothing to have parity with. This alone would justify rejection.

### N-03 (REOPENED): Bead contract does not exist
- Severity: **HIGH**
- Location: `.beads/vo-0qg/`
- The bead has no contract spec. You cannot claim to have "fixed" contract parity when there was never a contract to begin with.

---

## PHASE 2: Farley Engineering Rigor

### N-02 (REOPENED): Zero integration tests for the spawn path
- Severity: **HIGH**
- Location: `crates/vo-actor/tests/`
- `cargo test` shows 8 integration test files exist for crash replay, determinism, FSM terminal states — **none** test the `StartWorkflow → spawn_and_register → register` path.
- The 3 unit tests in start.rs:107-166 only test `validate_request`. The actual async spawn, persistence, and registration path (`handle_start_workflow` lines 8-26, `spawn_and_register` lines 65-86) has **zero** test coverage.
- `spawn_and_register` is the most critical 20 lines in the crate. It spawns an actor, persists metadata, and registers it in the orchestrator's active map. **Untested.**

### Function length audit
- `handle_start_workflow` (start.rs:8-26): 19 lines ✓
- `spawn_and_register` (start.rs:65-86): 22 lines ✓
- `build_args` (start.rs:41-63): 23 lines ✓
- `handle_heartbeat_expired` (heartbeat.rs:21-57): **37 lines** ✗ — **EXCEEDS 25-line limit**
- `build_recovery_args` (heartbeat.rs:67-82): 16 lines ✓

### Parameter count audit
- `handle_start_workflow` (start.rs:8-17): **7 parameters** ✗ — **EXCEEDS 5-parameter limit**
- `build_args` (start.rs:41-48): **6 parameters** ✗ — **EXCEEDS 5-parameter limit**

### N-06: Duplicate `InstanceArguments` construction — NOT FIXED
- Severity: **MEDIUM**
- Location: `start.rs:41-63` (`build_args`) and `heartbeat.rs:67-82` (`build_recovery_args`)
- These two functions construct `InstanceArguments` with identical fields, just from different sources. If `InstanceArguments` gains a field, both must be updated independently. This is a textbook DRY violation and a future maintenance landmine.
- **Required fix:** Extract a single `InstanceArguments::from_parts()` or a shared builder. The struct already has 12 fields — manually writing them out twice is unacceptable.

---

## PHASE 3: NASA-Level Functional Rust (The Big 6)

### N-07 (HALF-FIX from N-01): Persistence failure logged but not propagated — `let _ =` on `reply.send`
- Severity: **CRITICAL**
- Location: `start.rs:76-83`

The original `let _ = store.put_instance_metadata(metadata).await;` is now wrapped in `if let Err(e)` with `tracing::error!`. **This is the bare minimum fix and it is not sufficient.**

Here's the problem: if `persist_metadata` fails:
1. The instance IS spawned and live in the actor system.
2. The instance IS registered in `state.active`.
3. The caller receives `Ok(instance_id)`.
4. The metadata is **not** in the state store.
5. If the node crashes before the next heartbeat, **the instance is silently lost** — crash recovery will never find it.

The `tracing::error!` log message correctly says `"metadata persistence failed — instance is live but invisible to crash recovery"`. **The author acknowledged the exact severity of the bug in the log message and then chose to do nothing about it.** This is malpractice for a system whose entire value proposition is "guaranteed no lost transitions."

**Required fix:** `persist_metadata` failure should either:
- (a) Return `Err(StartError::PersistenceFailed)`, kill the spawned actor, and fail the RPC — clean rollback, or
- (b) Retry with backoff before admitting defeat.

Logging is not a recovery strategy. A system that silently accepts a state where the in-memory actor map disagrees with the persistent metadata store is **lying about its guarantees**.

### N-08: `let _ = reply.send(...)` — reply failures silently discarded
- Severity: **MEDIUM**
- Location: `start.rs:19`, `start.rs:25`, `mod.rs:41-50`, `mod.rs:93`, `mod.rs:96`
- Every single `reply.send()` result is discarded with `let _ =`. If the caller drops the reply port (timeout, crash), the orchestrator never knows the RPC failed. For `StartWorkflow` specifically, the caller may believe the instance was never created when it actually was — leading to duplicate spawn attempts.
- At minimum, the `StartWorkflow` path should log when the reply send fails, because the caller's view of the world is now inconsistent with the orchestrator's.

### N-09: `let _ =` on heartbeat persistence — same class of bug as N-01
- Severity: **HIGH**
- Location: `instance/handlers.rs:103`
- `let _ = store.put_heartbeat(...).await;` — identical pattern to the original N-01 bug. Heartbeat persistence failure is silently swallowed. If heartbeats stop being written, the heartbeat-expiry watcher will trigger **false crash recovery**, spawning a duplicate instance.
- This is the same bug class, in a different file. Was the author's fix for N-01 intentionally scoped to only one occurrence?

### N-10: Boolean parameter: `if let Ok(...)` hides failure semantics
- Severity: **LOW** (Big 6: Types as Documentation)
- Location: `heartbeat.rs:49-53`
- `if let Ok((actor_ref, _)) = WorkflowInstance::spawn_linked(...)` — the spawn failure is silently skipped. No log, no error, no metric. If recovery spawn fails, the instance simply stays dead with zero observability. Recovery should be loud when it fails.

### N-11: `OrchestratorMsg::StartWorkflow` has 6 data fields
- Severity: **MEDIUM** (Big 6: Parse, Don't Validate)
- Location: `orchestrator.rs:16-23`
- `namespace`, `instance_id`, `workflow_type`, `paradigm`, `input`, `reply` — 6 fields. The first 5 should be a `StartWorkflowRequest` struct. This is a flat tuple-enum variant masquerading as structured data.

### Unwrap/expect audit
- `start.rs:162`: `.expect("null actor spawned")` — in test code. Acceptable.
- `state.rs:181`: `.expect("null actor spawned")` — in test code. Acceptable.
- No production `unwrap()` or `expect()` found in the reviewed files. ✓

---

## PHASE 4: Ruthless Simplicity & DDD (Scott Wlaschin)

### N-12: Duplicate `NullActor` struct across test modules
- Severity: **LOW**
- Location: `start.rs:116-131` and `state.rs:147-162`
- Identical `NullActor` implementation copy-pasted into two test modules. This should be in a shared `test_support` module.
- Violates DRY and makes test maintenance brittle.

### N-13: `OrchestratorConfig.engine_node_id` is a raw `String`
- Severity: **LOW** (Newtypes)
- Location: `state.rs:14`
- `engine_node_id: String` — this is a domain identifier. It should be a `newtype EngineNodeId(String)` per the Newtypes rule. Same applies to `InstanceArguments.engine_node_id` and `InstanceArguments.workflow_type`.

### N-14: `workflow_type: String` is unwrapped in `InstanceArguments`
- Severity: **LOW** (Newtypes)
- Location: `instance.rs:18`
- Should be `WorkflowTypeId` or similar. This is passed to `registry.get_procedural(&wtype)` and `registry.get_definition(&wtype)` — a `String` has no domain semantics.

---

## PHASE 5: The Bitter Truth

### The Persistence Failure Debate

The author replaced `let _ = store.put_instance_metadata(metadata).await;` with:

```rust
if let Err(e) = persist_metadata(state, &args).await {
    tracing::error!(
        instance_id = id.as_str(),
        error = %e,
        "metadata persistence failed — instance is live but invisible to crash recovery"
    );
}
```

This reads like someone who understands the severity of the problem but chose the cheapest fix. The log message is well-written, but it's a **confession**, not a solution. In production, this log will scroll past in a sea of other errors. Nobody will notice that an instance is now invisible to crash recovery. The system's core guarantee — "no lost transitions" — is violated, and the only witness is a log line.

**The question this code cannot answer:** What happens 10 minutes later when the node crashes? The instance was live, transitions were happening, events were published to JetStream — but the metadata key was never written. Crash recovery scans KV for instance metadata, finds nothing, and **those transitions are orphans forever**. This is the exact failure mode the system is designed to prevent.

### The Duplicate Construction Problem

`build_args` (start.rs:49-62) and `build_recovery_args` (heartbeat.rs:68-81) construct the same 12-field struct. I count 12 field assignments written out twice. If someone adds field #13 to `InstanceArguments`, one of these will be missed, and there will be no compile-time error — only a silent runtime bug where one path has a default value and the other doesn't. This is preventable with a shared constructor.

### The Race Condition

Between `state.register(id.clone(), actor_ref)` (start.rs:84) and the completion of `persist_metadata`, there is no atomicity. If the process crashes between these two operations, the actor is in the active map (in-memory) but not in KV. On restart, the actor is gone from memory and KV says it never existed. However, since the register happens AFTER persist_metadata (lines 76-84), this specific race window is small. The bigger issue is that persist_metadata failure leaves the actor registered but un-persisted — and the caller is told success.

### Heartbeat.rs spawn failure — silent kill

In `heartbeat.rs:49-53`, if `spawn_linked` fails during recovery, nothing happens. No log. The in-flight guard is cleaned up, and the instance remains dead. The heartbeat watcher will fire again (since the heartbeat KV entry still exists), so recovery will be retried — but there's no observability into how many times recovery has been attempted and failed. This is a silent failure loop.

---

## Summary of All Defects

| ID | Severity | Phase | Title |
|----|----------|-------|-------|
| N-07 | **CRITICAL** | 3 | Persistence failure logged but not propagated — caller told success while crash recovery is broken |
| N-02 | **HIGH** | 2 | Zero integration tests for spawn_and_register path |
| N-03 | **HIGH** | 1 | Bead contract does not exist — parity cannot be verified |
| N-09 | **HIGH** | 3 | `let _ =` on heartbeat persistence in instance/handlers.rs:103 — same bug class as N-01 |
| N-06 | **MEDIUM** | 2 | Duplicate InstanceArguments construction not fixed |
| N-08 | **MEDIUM** | 3 | `let _ = reply.send()` everywhere — caller may have inconsistent view |
| N-11 | **MEDIUM** | 3 | StartWorkflow variant has 6 data fields — should be a struct |
| N-10 | **LOW** | 3 | Recovery spawn failure silently swallowed in heartbeat.rs:49 |
| N-12 | **LOW** | 4 | Duplicate NullActor test helper |
| N-13 | **LOW** | 4 | `engine_node_id` should be a newtype |
| N-14 | **LOW** | 4 | `workflow_type` should be a newtype |
| N-04 | ~~MEDIUM~~ | — | **FIXED** ✓ — AlreadyExists test exists |
| N-01 | ~~CRITICAL~~ | — | **HALF-FIXED** — log added, but caller still told success |

---

## Farley Hard Constraint Violations

| Constraint | Violation | Location |
|-----------|-----------|----------|
| Function ≤ 25 lines | `handle_heartbeat_expired` = 37 lines | heartbeat.rs:21-57 |
| Function ≤ 5 params | `handle_start_workflow` = 7 params | start.rs:8-17 |
| Function ≤ 5 params | `build_args` = 6 params | start.rs:41-48 |

---

## Mandatory Fixes Before Re-Review

1. **N-07**: `persist_metadata` failure must propagate as `Err(StartError::PersistenceFailed)` and the spawned actor must be killed. No exceptions. This is a correctness bug, not a style issue.
2. **N-02**: Write an integration test that exercises `handle_start_workflow` end-to-end: spawn actor, verify it's in `state.active`, verify metadata was persisted, verify `AlreadyExists` on duplicate.
3. **N-09**: Fix the `let _ =` on heartbeat persistence in `instance/handlers.rs:103`. Same pattern, same severity.
4. **N-06**: Extract shared `InstanceArguments` construction to eliminate the `build_args` / `build_recovery_args` duplication.
5. **N-08**: At minimum, log when `reply.send()` fails in the `StartWorkflow` path.
6. Farley constraint violations: refactor `handle_heartbeat_expired` (split into smaller functions), group parameters into structs for `handle_start_workflow` and `build_args`.

---

## Verdict

**STATUS: REJECTED**

The most critical fix (N-01) was treated with a band-aid. The author correctly identified that persistence failure means "instance is live but invisible to crash recovery" — and then chose to log it instead of failing the operation. For a system whose selling point is durability, this is disqualifying. The zero integration test situation persists. The duplicate construction persists. New `let _ =` patterns were found in related code. Come back when the persistence failure is a hard error, not a log line.
