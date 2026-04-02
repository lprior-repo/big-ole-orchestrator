# Black Hat Review — Bead wtf-7fe: MasterOrchestrator struct and OrchestratorState

**Reviewer:** Black Hat Reviewer
**Date:** 2026-03-23
**Scope:** `crates/wtf-actor/src/master/` (mod.rs, state.rs, registry.rs, handlers/)

---

## PHASE 1: Contract & Bead Parity

### Bead Requirement vs Implementation

| Requirement | Spec | Implementation | Verdict |
|---|---|---|---|
| MasterOrchestrator has `max_concurrent: usize` | ADR-006 line 46 | **MISSING** — struct is a zero-field unit struct (mod.rs:12) | FAIL |
| MasterOrchestrator has `storage: Arc<sled::Db>` | ADR-006 line 47 | **MISSING** — fields moved into `OrchestratorConfig` instead | FAIL |
| OrchestratorState has `instances: HashMap<String, (String, ActorRef<InstanceMsg>)>` | ADR-006 line 52 | **MUTATED** — now `HashMap<InstanceId, ActorRef<InstanceMsg>>` (state.rs:42). The `(String, ...)` tuple dropped. `String` replaced with newtype `InstanceId`. | DEVIATION (improvement) |
| OrchestratorState has `running_count: usize` | ADR-006 line 53 | **MISSING** — eliminated in favor of `active.len()` (state.rs:63) | DEVIATION (improvement) |
| Impl `Actor for MasterOrchestrator` with `pre_start` | Bead description | Present, `pre_start` initializes state (mod.rs:20-31) | PASS |
| Structs compile | Bead acceptance | `cargo check` passes | PASS |
| State initializes empty registry and 0 running count | Bead acceptance | `OrchestratorState::new()` creates `HashMap::new()` + `WorkflowRegistry::new()` (state.rs:52-58), `active_count()` returns 0 (state.rs:62-64) | PASS |

**Defect D1: `MasterOrchestrator` struct has zero fields.** ADR-006 specifies `max_concurrent: usize` and `storage: Arc<sled::Db>` as fields on the struct. The implementation instead uses a unit struct with `OrchestratorConfig` as `Arguments`. This is an **intentional architectural divergence** — the author moved config into the Ractor `Arguments` type, which is a valid pattern. However, it directly violates the ADR spec.

**Severity: LOW.** The author's pattern is *better* than the ADR (config is immutable after `pre_start` and doesn't pollute the struct). This is a documentation/ADR-lag issue, not a code defect. The ADR should be updated.

**Defect D2: No `orchestrator_config_default_*` values in bead spec.** The bead says nothing about `OrchestratorConfig` or its defaults (1000 max instances, "engine-local" node ID). These are invented beyond scope but are harmless defaults.

**Severity: INFO.**

**Defect D3: `OrchestratorState` diverges from ADR tuple type.** ADR says `HashMap<String, (String, ActorRef<InstanceMsg>)>`. Implementation is `HashMap<InstanceId, ActorRef<InstanceMsg>>`. The `(String, ActorRef)` tuple lost the workflow name.

**Severity: LOW.** The workflow name can be recovered from the actor's state or metadata. Dropping it from the registry is a design choice. But it means `handle_list_active` (list.rs:5-15) has to RPC-call every single actor to get status snapshots instead of being able to return lightweight `(name, id)` tuples. This is a latent performance defect that will bite at scale.

---

## PHASE 2: Farley Engineering Rigor

### Function Length Constraint (25 lines)

| Function | Location | Lines | Verdict |
|---|---|---|---|
| `MasterOrchestrator::handle` | mod.rs:33-55 | 22 lines | PASS |
| `handle_other_msg` | mod.rs:70-96 | 26 lines | **FAIL — 26 lines, over 25** |
| `handle_child_termination` | mod.rs:98-111 | 13 lines | PASS |
| `handle_start_workflow` | start.rs:8-26 | 18 lines | PASS |
| `validate_request` | start.rs:28-39 | 11 lines | PASS |
| `build_args` | start.rs:41-63 | 22 lines | PASS |
| `spawn_and_register` | start.rs:65-79 | 14 lines | PASS |
| `persist_metadata` | start.rs:81-95 | 14 lines | PASS |
| `handle_terminate` | terminate.rs:10-23 | 13 lines | PASS |
| `call_cancel` | terminate.rs:25-42 | 17 lines | PASS |
| `handle_signal` | signal.rs:8-27 | 19 lines | PASS |
| `handle_get_status` | status.rs:9-17 | 8 lines | PASS |
| `handle_list_active` | list.rs:5-15 | 10 lines | PASS |
| `handle_heartbeat_expired` | heartbeat.rs:14-52 | **38 lines** | **FAIL — 38 lines, over 25** |
| `fetch_metadata` | heartbeat.rs:54-60 | 6 lines | PASS |
| `build_recovery_args` | heartbeat.rs:62-77 | 15 lines | PASS |

**Defect D4: `handle_other_msg` (mod.rs:70-96) is 26 lines — 1 over the 25-line hard limit.**

**Severity: LOW.** Borderline violation. The match arms are all single-line dispatches. Fix: extract `handle_other_msg` into the handlers module alongside the other handler functions.

**Defect D5: `handle_heartbeat_expired` (heartbeat.rs:14-52) is 38 lines — 13 over the 25-line hard limit.**

**Severity: MEDIUM.** This function does too much: duplicate-check, metadata fetch, recovery-arg construction, spawn, and cleanup. It's a god-function hiding behind a wall of early returns. Split into `deduplicate_recovery`, `attempt_recovery`, and `cleanup_in_flight`.

### Functional Core / Imperative Shell

**PASS.** The `OrchestratorState` module (state.rs) is a pure-data container with pure-methods. All I/O (actor spawning, RPC calls, store persistence) is in the handlers and `mod.rs`. Clean separation.

### Parameter Count

**Defect D6: `handle_start_workflow` (start.rs:8-17) takes 8 parameters.** Hard limit is 5.

**Severity: MEDIUM.** This is a textbook sign that a parameter object is needed. All these params represent a single concept: a "start workflow request." The `OrchestratorMsg::StartWorkflow` variant already IS that parameter object, but the handler destructures it into positional params. Fix: accept the variant as a struct or use a dedicated `StartRequest` type.

**Defect D7: `build_args` (start.rs:41-63) takes 6 parameters.** Over the 5-parameter limit.

**Severity: LOW.** 1 over. But same root cause as D6 — this is a function that should take a struct, not a spread of primitives.

### Test Quality

11 tests across the master module:

- `new_state_is_empty` — Asserts behavior (count = 0). PASS.
- `has_capacity_when_empty` — Asserts behavior. PASS.
- `has_capacity_false_when_at_limit` — Asserts behavior (boundary). PASS.
- `get_returns_none_for_unknown_id` — Asserts behavior. PASS.
- `deregister_removes_entry` — Asserts behavior (no panic on missing). PASS.
- `orchestrator_config_default_max_instances` — Asserts default value. PASS.
- `orchestrator_config_default_node_id` — Asserts default value. PASS.
- `validate_request_rejects_when_at_capacity` — Tests validation logic. PASS.
- `validate_request_accepts_when_capacity_available` — Tests validation logic. PASS.
- `terminate_returns_not_found_for_unknown_instance` — Integration test via oneshot channel. PASS.
- `list_active_returns_empty_when_no_instances` — Tests empty-list path. PASS.

**Defect D8: No test for `register` + `get` round-trip.** The critical path (register an instance, then look it up) is untested.

**Severity: MEDIUM.** The acceptance criteria says "state initializes empty registry and 0 running count" — the tests cover initialization. But the *use* of the registry (register → get → deregister lifecycle) has zero coverage. You're testing the parking lot is empty, not that cars can park, be found, and leave.

**Defect D9: No test for `OrchestratorState::new(config).registry` being empty.** The bead says "initializes empty registry." `WorkflowRegistry::new()` defaults via `#[derive(Default)]`, but no test asserts the registry is empty at construction.

**Severity: LOW.** `Default` derive handles this, but an explicit test would catch a regression if someone adds pre-population.

**Defect D10: No test for `pre_start` returning `Ok(OrchestratorState)` with correct config.** The bead says "state initializes... pre_start initialization." The `pre_start` function is untested.

**Severity: MEDIUM.** `pre_start` is the contract entry point. Not testing it means the most important lifecycle transition is unverified.

**Defect D11: `handle_other_msg` has a wildcard `_ => {}` arm (mod.rs:94).** This silently swallows unhandled messages with zero logging.

**Severity: HIGH.** If a new `OrchestratorMsg` variant is added and someone forgets to handle it, the message is silently dropped. This is a data-loss vector. Every wildcard arm in an actor message handler should at minimum `tracing::warn!` about the unhandled message.

---

## PHASE 3: NASA-Level Functional Rust (The Big 6)

### 1. Make Illegal States Unrepresentable

**PASS with notes.**
- `InstanceId` is a proper newtype (not raw `String`). Good.
- `WorkflowParadigm` is an enum. Good.
- `OrchestratorConfig` uses `Option<>` for stores, which correctly represents "store may be absent." Good.

**Defect D12: `OrchestratorConfig.engine_node_id` is a raw `String`.** This should be `NamespaceId` or a dedicated `NodeId` newtype. Currently it's an unwrapped primitive in a domain model.

**Severity: LOW.** Consistent with the rest of the codebase (NamespaceId/InstanceId exist, NodeId does not). YAGNI says wait until there's logic that validates node IDs.

### 2. Parse, Don't Validate

**PASS.** `InstanceId::try_new()` exists for validated construction. `OrchestratorMsg` is parsed at the actor boundary (ractor handles deserialization).

### 3. Types as Documentation — Boolean Parameters

**PASS.** No boolean parameters found anywhere in the reviewed code.

### 4. Workflows as State Transitions

**Partial.** Instance lifecycle (start → active → terminated) is implicit in `register`/`deregister`. There's no explicit state enum for instance lifecycle within the orchestrator. The registry only tracks "active" or "not present."

**Defect D13: No `InstanceLifecycle` state enum.** The orchestrator treats instances as either "in the HashMap" or "not." There's no way to represent "spawning," "draining," "recovering" states that `handle_heartbeat_expired` clearly needs (hence the `in_flight_set` hack in heartbeat.rs:9-11).

**Severity: MEDIUM.** The `OnceLock<Mutex<HashSet<String>>>` global static in heartbeat.rs is a direct consequence of not having proper lifecycle states. This is mutable global state — the exact antithesis of functional Rust.

### 5. Newtypes for Domain Primitives

**PASS.** `InstanceId`, `NamespaceId`, `WorkflowParadigm` are all proper types. No raw `String` used as IDs in the message types.

### 6. Zero Panics / Unwrap / Expect

**Defect D14: `in_flight_set().lock().unwrap_or(false)` (heartbeat.rs:28-29).** The `unwrap_or(false)` means if the Mutex is poisoned, the recovery is silently skipped with a `false` return, which then causes the function to proceed as if this is a duplicate recovery and skip it entirely. A poisoned mutex means a thread panicked while holding the lock — the data is potentially corrupt. Silently swallowing this is hiding a critical error.

**Severity: HIGH.** Mutex poisoning = a thread died while mutating shared state. The correct response is to propagate the error or at minimum log at `tracing::error!` level. Returning `false` here means "pretend this recovery is already in-flight" which is a lie that causes real recoveries to be silently dropped.

**Defect D15: `in_flight_set().lock().map(|mut set| set.remove(&in_flight_key))` (heartbeat.rs:51).** If the mutex is poisoned, this `map` returns `None`, and the `let _ =` discards it. The key is never removed from the set. This means a single mutex poisoning permanently leaks the in-flight entry, and that instance can **never be recovered again**.

**Severity: HIGH.** This is a data-loss vector. Once poisoned, the HashSet accumulates entries forever. Each unique instance that triggers recovery during a poison event gets permanently stuck in the "already in-flight" state.

---

## PHASE 4: Strict DDD (Scott Wlaschin)

### CUPID Properties

| Property | Assessment |
|---|---|
| **Composable** | PARTIAL. State, config, and registry are separate types. Good. But `handle_heartbeat_expired` mixes deduplication, persistence, and spawning. |
| **Unix-philosophy** | FAIL. `handle_heartbeat_expired` does 5 things. The global static `in_flight_set` is a hidden singleton. |
| **Predictable** | FAIL. Wildcard `_ => {}` arm silently drops messages. Mutex poisoning silently skips recoveries. |
| **Idiomatic** | PASS. Uses `#[must_use]`, derives, standard patterns. |
| **Domain-based** | PARTIAL. Domain types are good. But `orchestrator_config_default_*` magic values (1000, "engine-local") are not domain-derived. Why 1000? Document the constraint. |

### No Option-based State Machines

**PASS.** No `Option<State>` pattern used for state machines.

### The Panic Vector

Already covered in Phase 3 (Defects D14, D15). Zero `unwrap()`/`expect()`/`panic!()` in non-test code. The `lock().unwrap_or()` and `lock().map()` patterns are subtler but equally dangerous.

**Defect D16: Unnecessary `let mut` in test.** `handle_terminate` takes `&mut OrchestratorState` but terminate.rs test (line 52-70) creates `let mut state`. This is justified — the handler signature requires `&mut`.

**Severity: INFO.** Not a defect.

---

## PHASE 5: The Bitter Truth

### Is This Code Actually Correct?

**Defect D17: `handle_list_active` (list.rs:5-15) makes N RPC calls sequentially.** For every active instance, it calls `handle_get_status`, which in turn does an `actor_ref.call()` with a 500ms timeout. If there are 100 active instances and one is slow, this blocks for 50 seconds. If one is dead, it blocks for 500ms.

**Severity: MEDIUM.** This is a latent performance and reliability defect. `list.rs` should use `tokio::join!` or `FuturesUnordered` for concurrent status queries.

**Defect D18: `handle_child_termination` (mod.rs:98-111) uses `active.iter().find()` — O(n) linear scan.** It matches by `ActorCell` ID. This is fine for the current scale (1000 instances max) but the ADR says `max_concurrent: 3`. If this ever scales up, this becomes a hot path.

**Severity: INFO.** Acceptable at current scale. But should use a secondary `HashMap<ActorId, InstanceId>` index if capacity increases.

**Defect D19: `handle_supervisor_evt` only handles `ActorTerminated`, not `ActorFailed`.** ADR-006 (lines 126-148) explicitly shows handling both `ActorTerminated` and `ActorFailed`. The implementation (mod.rs:57-67) only matches `ActorTerminated`. If a child actor panics, `ActorFailed` fires, and the instance stays in the registry forever — a zombie.

**Severity: HIGH.** This is a correctness defect that directly contradicts ADR-006. A crashed workflow instance will be permanently registered as "active" even though its actor is dead. This will:
  1. Incorrectly report it in `ListActive`
  2. Block capacity (no slot freed)
  3. Reject `StartWorkflow` with `AlreadyExists` if the same instance ID is retried

**Defect D20: No `ActorStarted` handler in supervision events.** ADR-006 (lines 139-143) shows incrementing `running_count` on `ActorStarted`. The implementation doesn't handle this. Not critical since it uses `active.len()`, but it means the ADR and implementation are out of sync.

**Severity: INFO.** Not a bug due to the `HashMap::len()` approach, but another ADR drift point.

### The Sniff Test

The code reads like it was written by someone who knows ractor well and made deliberate improvements over the ADR. The separation into handlers/ is clean. The use of newtypes is solid. But there are three "clever" patterns that raise my hackles:

1. **The global `OnceLock<Mutex<HashSet>>`** — This is a hidden singleton that exists because the author couldn't figure out how to track in-flight recoveries in the state. It's a classic "I'll just use a global" shortcut.

2. **The wildcard `_ => {}` message sink** — Sloppy. Every message deserves a destination.

3. **Missing `ActorFailed` handling** — This reads like "I implemented the happy path and called it done." The ADR literally has the code for `ActorFailed` and it was copy-pasted selectively.

---

## Summary of Defects

| ID | Severity | Phase | Description |
|---|---|---|---|
| D1 | LOW | 1 | MasterOrchestrator has zero fields, ADR says 2 |
| D2 | INFO | 1 | OrchestratorConfig defaults invented beyond spec |
| D3 | LOW | 1 | ADR tuple `(String, ActorRef)` simplified to just `ActorRef` |
| D4 | LOW | 2 | `handle_other_msg` 26 lines (1 over limit) |
| D5 | MEDIUM | 2 | `handle_heartbeat_expired` 38 lines (13 over limit) |
| D6 | MEDIUM | 2 | `handle_start_workflow` 8 parameters (3 over limit) |
| D7 | LOW | 2 | `build_args` 6 parameters (1 over limit) |
| D8 | MEDIUM | 2 | No test for register+get round-trip |
| D9 | LOW | 2 | No test for empty registry at construction |
| D10 | MEDIUM | 2 | `pre_start` untested |
| D11 | **HIGH** | 2 | Wildcard `_ => {}` silently drops unhandled messages |
| D12 | LOW | 3 | `engine_node_id` is raw `String`, not newtype |
| D13 | MEDIUM | 3 | No instance lifecycle state enum; causes global static hack |
| D14 | **HIGH** | 3 | Mutex poisoning silently skipped in recovery dedup |
| D15 | **HIGH** | 3 | Mutex poisoning permanently leaks in-flight entries |
| D16 | INFO | 4 | `let mut` in test is justified |
| D17 | MEDIUM | 5 | `handle_list_active` makes N sequential RPC calls |
| D18 | INFO | 5 | O(n) linear scan in `handle_child_termination` |
| D19 | **HIGH** | 5 | `ActorFailed` supervision event not handled — zombie instances |
| D20 | INFO | 5 | `ActorStarted` not handled (ADR drift, not a bug) |

---

## Verdict

**STATUS: REJECTED**

### Blocking Defects (must fix before merge):

1. **D11** — Wildcard `_ => {}` in message handler silently drops messages. Add `tracing::warn!`.
2. **D14/D15** — Global `OnceLock<Mutex<HashSet>>` in heartbeat.rs has two catastrophic failure modes on Mutex poisoning: silent skip of real recoveries, and permanent key leaks. Either eliminate the global (track in-flight state in `OrchestratorState`) or handle poisoning with error propagation.
3. **D19** — `ActorFailed` supervision event not handled. Crashed workflow instances become permanent zombies in the registry. This is a data-integrity bug.

### Must-Fix Before Next Review (non-blocking but expected):

4. **D5** — Split `handle_heartbeat_expired` (38 lines → ≤25).
5. **D6** — Introduce `StartRequest` struct to reduce `handle_start_workflow` to ≤5 params.
6. **D8/D10** — Add tests for register+get lifecycle and `pre_start`.

### Acknowledged Improvements Over ADR:

- Using `Arguments` type for config instead of struct fields is the correct ractor pattern.
- Replacing `(String, ActorRef)` with just `ActorRef` is cleaner (workflow name recoverable elsewhere).
- Replacing `running_count: usize` with `HashMap::len()` eliminates a class of desync bugs.
- Newtype `InstanceId` throughout is solid.

**The core structural design is sound. The rejection is for three correctness defects (D11, D14/D15, D19) that will cause silent data loss in production.**
