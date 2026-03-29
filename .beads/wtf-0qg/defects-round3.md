# Black Hat Review — Round 3 — Bead vo-0qg (spawn_workflow / spawn_and_register)

**Reviewer:** Black Hat
**Date:** 2026-03-23
**Files inspected:** 7 files across `vo-actor/src/` (start.rs, errors.rs, instance/handlers.rs, heartbeat.rs, state.rs, master/mod.rs, messages/instance.rs)
**Verdict:** `STATUS: REJECTED`

---

## Round 2 Defect Verification

| ID | Severity | Round 2 Status | Round 3 Verdict |
|----|----------|----------------|-----------------|
| N-07 | **CRITICAL** | MANDATORY FIX | **FIXED** ✓ — persistence failure kills actor AND returns `Err(StartError::PersistenceFailed)` |
| N-09 | **HIGH** | MANDATORY FIX | **FIXED** ✓ — heartbeat persistence error now logged at `error!` level |
| N-06 | **MEDIUM** | MANDATORY FIX | **FIXED** ✓ — shared `build_instance_args(seed)` with `InstanceSeed`, both paths use it, `build_args()` deleted |
| N-02 | **HIGH** | MANDATORY FIX | **NOT FIXED** — zero integration tests after 3 rounds |
| N-08 | **MEDIUM** | MANDATORY FIX | **NOT FIXED** — `let _ = reply.send()` still everywhere |
| Farley violations | **MEDIUM** | MANDATORY FIX | **NOT FIXED** — 8-param function and 37-line function persist |

The author fixed 3 of 6 mandatory items and introduced no regressions. The fixes are correct. But skipping the other 3 mandatory items after being told they were mandatory is not acceptable.

---

## PHASE 1: Contract & Bead Parity — FAILED

### N-03 (CARRIED): Bead contract still does not exist
- Severity: **HIGH**
- Location: `.beads/vo-0qg/`
- Third round. Still no `contract-spec.md` or `martin-fowler-tests.md`. The `implementation-round2.md` exists but it describes what was changed, not what should exist. A contract defines preconditions, postconditions, and invariants — none of which are documented anywhere.
- This has been flagged in Round 1 (C-06), Round 2 (N-03), and now Round 3. Three rounds of "no contract" is a governance failure.

---

## PHASE 2: Farley Engineering Rigor — FAILED

### N-02 (CARRIED): Zero integration tests for the spawn path
- Severity: **HIGH**
- Location: `crates/vo-actor/tests/`
- `spawn_and_register` is the single most critical function in the crate. It spawns an actor, persists metadata to the state store, and registers it in the orchestrator's active map. After THREE rounds of review, it has **zero** integration test coverage.
- The 3 unit tests in `start.rs:118-151` cover `validate_request` only. The async spawn + persist + register path is completely untested.
- The Round 2 fix for N-07 (kill actor on persist failure) is itself untested. There is no test that verifies: spawn succeeds → persist fails → actor is killed → error is returned. This is the kind of bug that gets re-introduced because nobody noticed the test was never written.

### Farley Hard Constraint Violations (CARRIED, NOT FIXED)

| Constraint | Violation | Location | Rounds Flagged |
|-----------|-----------|----------|----------------|
| Function ≤ 25 lines | `handle_heartbeat_expired` = 37 lines | heartbeat.rs:21-57 | **3 rounds** |
| Function ≤ 5 params | `handle_start_workflow` = 8 params | start.rs:8-17 | **3 rounds** |

The 8-param function is particularly galling because the author JUST introduced `InstanceSeed` to reduce parameter counts. But they only applied it to the internal `spawn_and_register` and `build_recovery_args` — not to the entry point `handle_start_workflow`. The 8 params are the same fields that now go into `InstanceSeed`. The refactoring was 80% done and the author stopped.

`handle_heartbeat_expired` was 37 lines in Round 1, 37 lines in Round 2, and 37 lines in Round 3. The extraction into `fetch_metadata` and `build_recovery_args` was already done. Splitting the early-return guard logic (lines 26-44) into a helper would bring it under 25. The author chose not to do this in 3 rounds.

### Function length audit

| Function | Lines | Status |
|----------|-------|--------|
| `handle_start_workflow` (start.rs:8-33) | 26 | ✗ EXCEEDS 25 by 1 |
| `spawn_and_register` (start.rs:48-71) | 24 | ✓ |
| `persist_metadata` (start.rs:73-90) | 18 | ✓ |
| `validate_request` (start.rs:35-46) | 12 | ✓ |
| `handle_heartbeat_expired` (heartbeat.rs:21-57) | 37 | ✗ EXCEEDS 25 by 12 |
| `fetch_metadata` (heartbeat.rs:59-65) | 7 | ✓ |
| `build_recovery_args` (heartbeat.rs:67-75) | 9 | ✓ |
| `handle_heartbeat` (handlers.rs:101-108) | 8 | ✓ |

Note: `handle_start_workflow` went from 26 lines in Round 2 to 26 lines now. It was 19 lines in Round 1 before the persist_metadata error handling was added. The growth is justified by the fix, but it's still over the limit.

---

## PHASE 3: NASA-Level Functional Rust (The Big 6) — PARTIAL PASS

### N-07: VERIFIED FIXED ✓
- **start.rs:59-68**: `persist_metadata` failure correctly:
  1. Logs structured error with `instance_id`, `namespace`, `error` fields ✓
  2. Calls `actor_ref.stop(Some("metadata persistence failed".into()))` to kill the spawned actor ✓
  3. Returns `Err(StartError::PersistenceFailed(e.to_string()))` ✓
  4. Never reaches `state.register()` — the actor is never in `state.active` ✓
- **errors.rs:15-16**: `PersistenceFailed(String)` variant with correct `#[error(...)]` message ✓
- The ordering is correct: spawn → persist → (if fail: kill + err) → (if ok: register + ok) ✓
- This is a proper fix. The author correctly identified that registration must only happen after successful persistence.

### N-09: VERIFIED FIXED ✓
- **instance/handlers.rs:101-108**: `let _ =` replaced with `if let Err(e) { tracing::error!(...) }` ✓
- Error logged at `error!` level with `error` field ✓
- Heartbeat is best-effort — instance continues after failure is acceptable ✓

### N-06: VERIFIED FIXED ✓
- **state.rs:93-108**: `build_instance_args(seed: InstanceSeed) -> InstanceArguments` is the single source of truth ✓
- **start.rs:23-30**: Fresh spawn constructs `InstanceSeed` + calls `state.build_instance_args(seed)` ✓
- **heartbeat.rs:67-75**: Recovery constructs `InstanceSeed` from metadata + calls `state.build_instance_args(seed)` ✓
- **messages/instance.rs:37-43**: `InstanceSeed` bundles per-instance identity fields ✓
- `build_args()` is fully deleted — `grep` confirms zero matches ✓
- No duplicate construction sites remain ✓

### N-15 (NEW): `let _ =` on cancellation event publish — same bug class as N-07
- Severity: **HIGH**
- Location: `instance/handlers.rs:124`
- ```rust
  if let Some(store) = &state.args.event_store {
      let _ = store.publish(
          &state.args.namespace,
          &state.args.instance_id,
          event,
      ).await;
  }
  ```
- The author was IN THIS FILE fixing N-09. They fixed `let _ =` on heartbeat (line 103) but left `let _ =` on the cancellation event publish (line 124). Same file. Same bug class. Different line.
- **Impact**: If cancellation event publish fails:
  1. Caller receives `Ok(())` — cancellation "succeeded"
  2. Instance stops via `myself_ref.stop()` (line 133)
  3. Cancellation event is NOT in JetStream
  4. On crash recovery, instance resumes without the cancellation event
  5. Instance continues executing from last checkpoint — but caller thinks it's cancelled
- This is a silent state divergence between in-memory actor and durable event log. It is the EXACT same failure mode as N-07 (the one the author just fixed). The author demonstrated they understand this bug class and then left another instance of it sitting in the same function.
- **Required fix**: At minimum, log at `error!` level. Ideally, propagate the error so the caller knows cancellation wasn't durable. If the caller must receive a definitive answer, retry before admitting failure.

### N-08 (CARRIED): `let _ = reply.send()` — caller view inconsistency
- Severity: **MEDIUM** (carried from Round 2)
- `start.rs:19`, `start.rs:32`: RPC reply failures silently discarded.
- In the Ractor model, a dropped reply port means the caller timed out. The orchestrator's state is still consistent (the instance exists or doesn't). So the caller's retry will correctly get `AlreadyExists` or succeed on a new attempt. This is acceptable for Ractor's architecture but should be documented.

### Additional `let _ = store.publish` instances (out of scope, noted for awareness)

Found in `instance/procedural_utils.rs`:
- Line 73: `let _ =` on `InstanceCompleted` event publish
- Line 91: `let _ =` on `InstanceFailed` event publish

These are the same bug class as N-15 but in files outside the 7 specified for this review. Flagged for the next review cycle.

### Unwrap/expect audit

- `start.rs:147`: `.expect("null actor spawned")` — test code. Acceptable ✓
- `state.rs:203`: `.expect("null actor spawned")` — test code. Acceptable ✓
- No production `unwrap()` or `expect()` found in the reviewed files ✓

---

## PHASE 4: Ruthless Simplicity & DDD (Scott Wlaschin) — PARTIAL PASS

### N-06: VERIFIED FIXED ✓
- The `InstanceSeed` + `build_instance_args` pattern is clean DDD: identity data (seed) is separated from infrastructure wiring (state). Good.

### N-12 (CARRIED): Duplicate `NullActor` across test modules
- Severity: **LOW** (carried from Round 2)
- Location: `start.rs:101-116` and `state.rs:169-184`
- Identical `NullActor` struct and `Actor` impl in two test modules. Third round.
- `vo-actor/src/` has no `test_support` module. This should be one.

### N-13/N-14 (CARRIED): Missing newtypes for domain identifiers
- Severity: **LOW** (carried from Round 2)
- `engine_node_id: String` — should be `EngineNodeId(String)`
- `workflow_type: String` — should be `WorkflowTypeId(String)` or similar
- `OrchestratorConfig.engine_node_id`, `InstanceArguments.engine_node_id`, `InstanceArguments.workflow_type`
- The codebase already has `InstanceId`, `NamespaceId`, `ActivityId` as newtypes — the pattern is established.

### N-10 (CARRIED): Recovery spawn failure silently swallowed
- Severity: **LOW** (carried from Round 2, downgraded)
- Location: `heartbeat.rs:49-53`
- `if let Ok((actor_ref, _)) = WorkflowInstance::spawn_linked(...)` — error branch does nothing.
- No log. No metric. The in-flight guard is cleaned up (line 56), and the heartbeat watcher will retry. But there's zero observability into recovery failures. Add at minimum a `tracing::warn!` in the error branch.

---

## PHASE 5: The Bitter Truth — FAILED

### The Three Fixes Are Genuinely Good

I'll give credit where it's due. N-07, N-09, and N-06 are correctly fixed. The N-07 fix in particular is well-structured: spawn → persist → (fail: kill + error) → (ok: register). The `InstanceSeed` pattern for N-06 is clean and eliminates a real maintenance hazard. The `build_args()` function is confirmed deleted. These are not band-aids — they're proper fixes.

### But the Author Only Fixed What They Were Told To

The author fixed exactly the 3 items explicitly listed in the Round 3 instructions:
> - N-07: persist_metadata failure now returns Err(StartError::PersistenceFailed) and kills spawned actor
> - N-09: heartbeat `let _ =` on put_heartbeat now logs error
> - N-06: deduplicated InstanceArguments construction

They did NOT fix the other 3 mandatory items from Round 2. They did NOT sweep for the same `let _ =` pattern in the file they were editing. They did NOT address the Farley violations that have been flagged for 3 consecutive rounds.

### The `let _ =` on Cancellation Is Inexcusable

The author was editing `instance/handlers.rs` to fix N-09 (heartbeat persistence). They changed line 103 from `let _ = store.put_heartbeat(...)` to `if let Err(e) = ... { tracing::error!(...) }`. Then they scrolled up 20 lines to `handle_cancel` and did NOT apply the same fix to `let _ = store.publish(...)` on line 124.

They demonstrated they understand the bug class. They were in the exact file. They applied the fix to one instance and left the other. This is either:
- Laziness (didn't read the rest of the file)
- Carelessness (didn't recognize the pattern)
- Apathy (knew about it and chose not to fix it)

None of these are acceptable for a durable execution engine.

### Three Rounds of No Integration Tests

`spawn_and_register` was flagged as having zero test coverage in Round 1. It was re-flagged as mandatory in Round 2. It's now Round 3 and there are still zero integration tests. The N-07 fix (kill actor on persist failure) is a critical behavior change that itself has no test verifying the kill-and-return-error semantics. How does the author know the fix works? They typed it and checked `cargo check`. That's not testing — that's compilation.

### The Farley Violations Show Disrespect for Process

I've flagged `handle_heartbeat_expired` (37 lines) and `handle_start_workflow` (8 params) for THREE rounds. The author introduced `InstanceSeed` specifically to reduce parameter counts but didn't apply it to the entry point. The function that CALLS `spawn_and_register` still has 8 params — the same params that now go into `InstanceSeed`. The refactoring is 80% complete and the author stopped.

---

## Summary of All Defects

| ID | Severity | Phase | Status | Title |
|----|----------|-------|--------|-------|
| N-07 | ~~CRITICAL~~ | 3 | **FIXED** ✓ | Persistence failure kills actor AND returns error |
| N-09 | ~~HIGH~~ | 3 | **FIXED** ✓ | Heartbeat persistence error logged |
| N-06 | ~~MEDIUM~~ | 4 | **FIXED** ✓ | Shared InstanceArguments construction via InstanceSeed |
| N-15 | **HIGH** | 3 | **NEW** | `let _ =` on cancellation event publish — same file as N-09 fix |
| N-02 | **HIGH** | 2 | **CARRIED** (3rd round) | Zero integration tests for spawn_and_register |
| N-03 | **HIGH** | 1 | **CARRIED** (3rd round) | Bead contract does not exist |
| N-08 | **MEDIUM** | 3 | **CARRIED** | `let _ = reply.send()` everywhere |
| N-10 | **LOW** | 3 | **CARRIED** | Recovery spawn failure silently swallowed |
| N-12 | **LOW** | 4 | **CARRIED** | Duplicate NullActor test helper |
| N-13 | **LOW** | 4 | **CARRIED** | `engine_node_id` should be newtype |
| N-14 | **LOW** | 4 | **CARRIED** | `workflow_type` should be newtype |
| N-04 | ~~MEDIUM~~ | — | **FIXED** ✓ (Round 2) | AlreadyExists test exists |

### Farley Hard Constraint Violations

| Constraint | Violation | Location | Rounds Flagged |
|-----------|-----------|----------|----------------|
| Function ≤ 25 lines | `handle_heartbeat_expired` = 37 lines | heartbeat.rs:21-57 | **3** |
| Function ≤ 25 lines | `handle_start_workflow` = 26 lines | start.rs:8-33 | **1** (new) |
| Function ≤ 5 params | `handle_start_workflow` = 8 params | start.rs:8-17 | **3** |

---

## Mandatory Fixes Before Round 4

1. **N-15**: Fix `let _ = store.publish(...)` in `handle_cancel` (instance/handlers.rs:124). The author was in this file. The bug class is identical to N-07 which they just fixed. No excuses.
2. **N-02**: Write an integration test for `spawn_and_register`. Verify: (a) success path registers instance and persists metadata, (b) persist failure kills actor and returns error, (c) duplicate ID returns `AlreadyExists`. Three rounds of zero coverage is unacceptable.
3. **Farley violations**: Either fix them or explain why they can't be fixed. `handle_start_workflow` can accept a `StartWorkflowRequest` struct (3 params: myself, state, request). `handle_heartbeat_expired` can extract the early-return guards into a helper.

---

## Verdict

**STATUS: REJECTED**

Three of three targeted fixes are correct. The N-07 fix in particular is well-structured and properly ordered. The `InstanceSeed` pattern is clean. But the author only did what they were explicitly told and stopped. They left the same `let _ =` bug class sitting 20 lines above their N-09 fix. They skipped 3 of 6 mandatory items. They've ignored Farley violations for 3 rounds. They have zero integration tests for the most critical function in the crate after 3 rounds of review.

The fixes are good. The discipline is not. Come back when the author sweeps the entire file for the patterns they just fixed, writes the integration tests that were mandatory two rounds ago, and either fixes the Farley violations or justifies why they can't be fixed.
