# Black Hat Review — Bead wtf-0qg: spawn_workflow method

**Reviewer:** Black Hat  
**Date:** 2026-03-23  
**Verdict:** REJECTED  
**Files Inspected:**
- `crates/wtf-actor/src/master/handlers/start.rs` (122 lines)
- `crates/wtf-actor/src/master/mod.rs` (111 lines)
- `crates/wtf-actor/src/master/handlers/mod.rs` (13 lines)
- `crates/wtf-actor/src/master/state.rs` (156 lines)
- `crates/wtf-actor/src/messages/instance.rs` (111 lines)
- `crates/wtf-actor/src/messages/errors.rs` (37 lines)
- `crates/wtf-actor/src/instance/actor.rs` (83 lines)
- `crates/wtf-actor/src/master/handlers/heartbeat.rs` (77 lines)

---

## PHASE 1: Contract & Bead Parity — FAILED

### C-01: Function name mismatch (MUST FIX)
| Field | Bead Spec | Actual |
|-------|-----------|--------|
| Name | `spawn_workflow` | `spawn_and_register` |

**start.rs:65** — The function was renamed from `spawn_workflow` to `spawn_and_register` without updating the bead. Rename the bead or rename the function. Pick one.

### C-02: Return type mismatch (MUST FIX)
| Field | Bead Spec | Actual |
|-------|-----------|--------|
| Return | `Result<ActorRef<InstanceMsg>, ActorProcessingErr>` | `Result<InstanceId, StartError>` |

**start.rs:69-78** — The bead says the caller receives an `ActorRef<InstanceMsg>` to interact with the spawned actor. The actual code consumes the ref into `state.register()` at line 77 and returns only the `InstanceId`. The caller cannot send messages to the spawned instance. This is a **contract violation** — the bead acceptance criteria says "WorkflowInstance actor spawns linked under MasterOrchestrator supervision" which is true, but the signature divergence means no caller can verify the ref.

### C-03: Parameter signature mismatch (MUST FIX)
| Field | Bead Spec | Actual |
|-------|-----------|--------|
| Self | `&self` method | Free function |
| Params | `(name: String, invocation_id: String, input: Vec<u8>)` | `(myself, state, args: InstanceArguments)` |

**start.rs:65-68** — The bead specifies a method with primitive params. The actual takes a pre-built `InstanceArguments` struct. The struct approach is architecturally superior, but the bead is stale and misleading.

### C-04: Config type mismatch (MUST FIX)
| Field | Bead Spec | Actual |
|-------|-----------|--------|
| Config | `InstanceConfig` | `InstanceArguments` |

**start.rs:3** — `InstanceArguments` is the actual type. No `InstanceConfig` exists anywhere in the codebase. The bead references a type that was never created.

### C-05: spawn_linked invocation mismatch (MUST FIX)
| Field | Bead Spec | Actual |
|-------|-----------|--------|
| Actor | `WorkflowInstance::new(name)` | `WorkflowInstance` (unit struct) |
| Supervisor | `myself.clone().into()` | `myself.into()` |

**start.rs:72-73** — `WorkflowInstance` is a unit struct (see `actor.rs:15`), not constructed with `::new(name)`. The `name` is passed as the first arg to `spawn_linked`. The supervisor conversion also omits `.clone()` — though this works because `myself` is passed by value.

### C-06: No contract-spec.md or martin-fowler-tests.md (MUST FIX)
**`.beads/wtf-0qg/`** — Directory is empty. No contract spec. No test plan. No acceptance criteria beyond the bd description. This is a governance failure.

### C-07: Missing test for AlreadyExists branch (MUST FIX)
**start.rs:28-39** — `validate_request` has two error branches: `AtCapacity` (line 29-33, tested) and `AlreadyExists` (line 35-37, **NOT TESTED**). The duplicate-instance guard has zero test coverage. Anyone can refactor it away without a test screaming.

---

## PHASE 2: Farley Engineering Rigor — FAILED

### F-01: Zero integration tests for actual spawn (CRITICAL)
**start.rs:65-79** — `spawn_and_register` is the most dangerous function in this file (it spawns real actors, writes to state store, mutates orchestrator state). It has **zero test coverage**. The `tests/` directory has 9 integration test files covering FSM crash replay, procedural workflows, timers — but **nothing** for the orchestrator spawn path. You can delete `spawn_and_register` tomorrow and no test fails.

### F-02: handle_start_workflow exceeds 5-parameter limit (WARN)
**start.rs:8-17** — 8 parameters. While this mirrors the `StartWorkflow` message struct fields, the function should accept the message directly or a validated struct. Eight parameters is a code smell that this is message-glue, not a well-factored function.

### F-03: Test quality — only validates validation (WARN)
**start.rs:97-121** — Tests only cover `validate_request`. Zero tests for `build_args`, `spawn_and_register`, or `persist_metadata`. The unit tests assert behavior correctly (good), but the tested surface is ~30% of the code.

---

## PHASE 3: NASA-Level Functional Rust (Big 6) — FAILED

### N-01: Silently swallowed metadata persistence error (CRITICAL)
**start.rs:94** — `let _ = store.put_instance_metadata(metadata).await;`

This is the single worst line in this file. The metadata is written to the state store so that crash recovery can re-spawn the instance. If this write fails, the instance is live and registered in memory but invisible to recovery. On orchestrator crash, this instance becomes an **orphan** — no heartbeat, no recovery path, no way to clean it up. This is a **data loss vector** in a durable execution engine.

**Required:** Either propagate the error (make `persist_metadata` return `Result`) or log at `error!` level with the instance_id. Silent `let _ =` on durability writes is unforgivable.

### N-02: Missing newtype for workflow_type (WARN)
**start.rs:14**, **instance.rs:19** — `workflow_type: String` appears in `InstanceArguments`, `handle_start_workflow`, `build_args`, `heartbeat.rs:67`, and `InstanceMetadata`. This is a domain concept masquerading as a primitive. Should be `WorkflowType` newtype.

### N-03: Missing newtype for engine_node_id (WARN)
**state.rs:14** — `engine_node_id: String` in `OrchestratorConfig`. Should be a newtype `EngineNodeId`.

### N-04: TOCTOU-safe but fragile guard pattern (INFO)
**start.rs:18-21, 65-78** — `validate_request` checks `state.active.contains_key(id)` before `spawn_and_register` inserts via `state.register(id, ref)`. This is safe only because Ractor processes messages sequentially per actor. If this assumption ever breaks (e.g., spawning on a different actor), the race window opens. A comment documenting this assumption would cost nothing.

---

## PHASE 4: Strict DDD (Scott Wlaschin) — FAILED

### D-01: spawn_and_register mixes three responsibilities (MUST FIX)
**start.rs:65-79** — One function does three things:
1. Spawns an actor (line 72-74)
2. Persists metadata to external store (line 76)
3. Registers in local state (line 77)

If step 2 fails (silently), step 3 still executes. If step 3 fails (it can't — HashMap::insert can't fail), you'd have an orphaned actor. The function violates Single Responsibility and has no compensating transaction.

### D-02: heartbeat.rs duplicates spawn+register logic (WARN)
**heartbeat.rs:45-48** — Recovery path has its own `spawn_linked` + `register` call that duplicates `spawn_and_register` from start.rs. Duplicated orchestration logic will diverge over time. This should call the same function.

### D-03: Option-based configuration in InstanceArguments (INFO)
**instance.rs:23-29** — `event_store: Option<Arc<dyn EventStore>>`, `state_store: Option<...>`, `task_queue: Option<...>`, `snapshot_db: Option<...>`. Four optional fields. An instance without an event store cannot persist events. An instance without a state store cannot heartbeat. These are not truly optional — they're "not configured yet." A builder pattern with compile-time guarantees would be superior.

---

## PHASE 5: The Bitter Truth — FAILED

### B-01: Actor name collision is unguarded at spawn level (WARN)
**start.rs:71** — `let name = format!("wf-{}", id.as_str());`

The `validate_request` guard prevents duplicate IDs in `state.active`, but if the actor name collides at the ractor level (e.g., from a concurrent recovery spawn), `spawn_linked` itself may error. The error is mapped to `StartError::SpawnFailed`, which is correct. But the error message from ractor won't indicate "name collision" — it'll be a generic spawn failure, making debugging harder.

### B-02: No observability on the critical spawn path (WARN)
**start.rs:65-79** — `spawn_and_register` has zero `tracing` instrumentation. The only logging is in `WorkflowInstance::pre_start` (actor.rs:28-33). If spawn fails, there's no structured log with the instance_id, namespace, or workflow_type. In production, you'll be debugging blind.

### B-03: The bead description is fiction (CRITICAL)
The bd description reads like it was written before the code existed and never updated. Five of seven signature fields are wrong. The referenced type `InstanceConfig` doesn't exist. The method name is wrong. This bead is a liability — any future developer reading it will be misled.

---

## Defect Summary

| ID | Severity | Phase | Description |
|----|----------|-------|-------------|
| C-01 | MUST FIX | 1 | Function name mismatch: `spawn_workflow` vs `spawn_and_register` |
| C-02 | MUST FIX | 1 | Return type mismatch: bead says `ActorRef<InstanceMsg>`, actual returns `InstanceId` |
| C-03 | MUST FIX | 1 | Parameter signature completely different from bead |
| C-04 | MUST FIX | 1 | `InstanceConfig` type doesn't exist; actual is `InstanceArguments` |
| C-05 | MUST FIX | 1 | `spawn_linked` invocation doesn't match bead |
| C-06 | MUST FIX | 1 | No contract-spec.md or martin-fowler-tests.md in bead directory |
| C-07 | MUST FIX | 1 | Missing test for `AlreadyExists` validation branch |
| F-01 | CRITICAL | 2 | Zero integration tests for `spawn_and_register` |
| F-02 | WARN | 2 | 8 parameters exceeds Farley 5-param limit |
| F-03 | WARN | 2 | Only 30% of code surface is tested |
| N-01 | CRITICAL | 3 | `let _ =` on metadata persistence — silent data loss vector |
| N-02 | WARN | 3 | `workflow_type: String` should be newtype |
| N-03 | WARN | 3 | `engine_node_id: String` should be newtype |
| N-04 | INFO | 3 | TOCTOU guard relies on undocumented Ractor single-threaded assumption |
| D-01 | MUST FIX | 4 | `spawn_and_register` mixes spawn + persist + register with no compensating transaction |
| D-02 | WARN | 4 | Heartbeat recovery duplicates spawn logic from start.rs |
| D-03 | INFO | 4 | Option-based config in InstanceArguments masks required dependencies |
| B-01 | WARN | 5 | Actor name collision produces opaque error |
| B-02 | WARN | 5 | Zero tracing on critical spawn path |
| B-03 | CRITICAL | 5 | Bead description is stale fiction — 5/7 signature fields wrong |

## Verdict

**STATUS: REJECTED**

7 MUST FIX defects, 3 CRITICAL defects, 6 WARN-level defects, 2 INFO-level defects.

The code itself is not terrible — it's clean, short functions, reasonable decomposition. But the bead contract is **completely divorced from reality** (5/7 fields wrong), the most critical line (metadata persistence) silently swallows errors creating an orphan risk in a **durable execution engine**, and the spawn path has **zero integration tests**. This is a durability system where the "persist" step is fire-and-forget. That is unacceptable.

**Mandatory before re-review:**
1. Update bead description to match actual implementation, OR rewrite code to match bead.
2. Create `contract-spec.md` and `martin-fowler-tests.md` in `.beads/wtf-0qg/`.
3. Fix N-01: `persist_metadata` must return `Result` or log at `error!` level. Silent `let _ =` on durability writes is a firing offense.
4. Write integration test for `spawn_and_register` that verifies actor spawns, registers in state, and persists metadata.
5. Write unit test for `AlreadyExists` validation branch.
6. Extract `spawn_and_register` to be reusable from `heartbeat.rs` recovery path (D-02).
