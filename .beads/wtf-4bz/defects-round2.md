# Black Hat Review — Round 2: Bead vo-4bz (capacity_check method)

**Reviewed:** 2026-03-23
**Reviewer:** black-hat-reviewer (glm-5-turbo)
**Round:** 2
**Files inspected:**
- `crates/vo-actor/src/master/state.rs` (197 lines)
- `crates/vo-actor/src/master/handlers/start.rs` (167 lines)
- `crates/vo-actor/src/master/mod.rs` (122 lines)
- `crates/vo-actor/src/master/handlers/status.rs` (21 lines)
- `crates/vo-actor/src/messages/errors.rs` (44 lines)
- `crates/vo-actor/src/messages/orchestrator.rs` (73 lines)
- `beads/master-orchestrator.json` (bead spec)
- `docs/adr/ADR-006-master-orchestrator-hierarchy.md`
- `.beads/vo-4bz/defects.md` (Round 1 findings)
- `.beads/vo-4bz/implementation.md` (Round 1 fixes)
- `crates/vo-actor/tests/procedural_ctx_start_at_zero.rs` (NullActor pattern source)

---

## Verification: Round 1 Defects

### R1-C1: Method signature mismatch (has_capacity vs capacity_check) — VERIFIED FIXED

`defects.md` line 15-17: ACCEPTED, documented. The method lives on `OrchestratorState` (line 68 of `state.rs`), not `MasterOrchestrator`. Paper trail is complete. The deviation is architecturally superior — pure state query, no actor reference dependency. **PASS.**

### R1-C2: running_count eliminated — VERIFIED FIXED

`defects.md` line 19-21: ACCEPTED, documented. Count is derived from `HashMap::len()` at `state.rs:69`. Grep confirms zero occurrences of `running_count` in the crate. **PASS.**

### R1-D-1: Vocabulary drift — VERIFIED DOCUMENTED

`defects.md` line 79-81: ACKNOWLEDGED. `implementation.md` lines 87-93 declare canonical vocabulary table. Code uses `has_capacity`/`active.len()`/`max_instances`. ADR-006 and bead spec still use the old names — acknowledged as housekeeping. **PASS (deferred).**

### R1-Missing boundary test: max_instances==1 with 1 active — VERIFIED FIXED

`state.rs:175-184`: Test `has_capacity_false_when_exactly_one_at_max_one` exists. It:
1. Creates config with `max_instances: 1` (line 165-173)
2. Spawns a `NullActor` to get a valid `ActorRef<InstanceMsg>` (line 179-181)
3. Registers the instance (line 182)
4. Asserts `!state.has_capacity()` (line 183)

**I ran this test. It passes.** `cargo test -p vo-actor --lib` → 68 tests, 0 failures. **PASS.**

---

## PHASE 1: Contract & Bead Parity — PASS

Bead spec (`beads/master-orchestrator.json` line 13-15):
```json
"fn capacity_check(&self, state: &OrchestratorState) -> bool {
    state.running_count < self.max_concurrent
}"
```

Implementation (`state.rs:68-70`):
```rust
pub fn has_capacity(&self) -> bool {
    self.active.len() < self.config.max_instances
}
```

Semantic equivalence is complete:
- `capacity_check` → `has_capacity` (renamed, paper-trailed)
- `state.running_count` → `self.active.len()` (derived, superior)
- `self.max_concurrent` → `self.config.max_instances` (more descriptive)

All deviations documented and accepted in Round 1. Contract intent is fully realized.

### C-4 (from Round 1): Missing contract.md / martin-fowler-tests.md — STILL DEFERRED

Second round. No contract.md or martin-fowler-tests.md exists in `.beads/vo-4bz/`. The `implementation.md` line 130 says "Deferred to housekeeping; implementation.md created instead." This is now the second review cycle. Housekeeping is a euphemism for "I won't do it." **Noted, not blocking — the code is correct and the implementation.md adequately documents the deviation.**

---

## PHASE 2: Farley Engineering Rigor — PASS (with findings)

### Hard Constraints

| Function | Lines | Params | Under 25? | Under 5? |
|----------|-------|--------|-----------|----------|
| `has_capacity()` | 3 | 1 | ✅ | ✅ |
| `active_count()` | 3 | 1 | ✅ | ✅ |
| `validate_request()` | 10 | 2 | ✅ | ✅ |
| `build_args()` | 14 | 6 | ✅ | ❌ 6 params |
| `spawn_and_register()` | 17 | 3 | ✅ | ✅ |
| `persist_metadata()` | 15 | 2 | ✅ | ✅ |

### FINDING E-3 (INFO, pre-existing): `handle_start_workflow` has 8 parameters

`start.rs:8-17`: `handle_start_workflow` has 8 parameters. Farley says 5 max. Clippy also flags it (default limit is 7). **Pre-existing — not introduced by this bead. Noted for future refactor.**

### FINDING E-4 (INFO, pre-existing): `build_args` has 6 parameters

`start.rs:41-47`: Exceeds the 5-parameter Farley limit. **Pre-existing — not introduced by this bead.**

### Test Quality: PASS

All tests assert behavior (WHAT), not implementation details (HOW):

| Test | Asserts |
|------|---------|
| `new_state_is_empty` | `active_count() == 0` |
| `has_capacity_when_empty` | `has_capacity() == true` |
| `has_capacity_false_when_at_limit` | `has_capacity() == false` (max=0) |
| `has_capacity_false_when_exactly_one_at_max_one` | `has_capacity() == false` (max=1, active=1) |
| `get_returns_none_for_unknown_id` | `get() == None` |
| `deregister_removes_entry` | no panic + count unchanged |
| `validate_request_rejects_when_at_capacity` | `is_err()` |
| `validate_request_accepts_when_capacity_available` | `is_ok()` |
| `validate_request_rejects_when_instance_already_exists` | `matches! AlreadyExists(_)` |

No implementation-detail assertions. Good.

### FINDING E-5 (INFO): Missing positive boundary test

No test for `max_instances == 1` with 0 active → `true`. We have the negative case (1 active → false) but not the positive case for the same boundary. The logic `0 < 1 = true` is trivially correct, so this is cosmetic, not a real risk.

### Pure Function Separation: PASS

- `has_capacity()` — pure: `&self` → `bool`, no I/O, no side effects. ✅
- `validate_request()` — pure: references in, `Result` out, no I/O. ✅
- `build_args()` — pure: data transformation only. ✅
- `spawn_and_register()` — imperative shell: I/O (spawn, persist). Correctly separated. ✅
- `persist_metadata()` — imperative shell: I/O only. Correctly separated. ✅

Functional core / imperative shell boundary is clean.

---

## PHASE 3: NASA-Level Functional Rust (Big 6) — PASS

### 1. Illegal states unrepresentable: ✅ PASS

Capacity derived from `HashMap::len()` — impossible for running count to disagree with the actual instance collection. This is architecturally superior to the bead spec's manual `running_count` field, which would have created an invariant violation risk.

### 2. Parse, Don't Validate: N/A

No external data parsing in the capacity check path.

### 3. Types as documentation: ✅ PASS

- `max_instances: usize` — clear semantic meaning
- `active: HashMap<InstanceId, ActorRef<InstanceMsg>>` — type-safe registry
- No boolean parameters anywhere in the capacity check path
- `#[must_use]` on `has_capacity()` and `active_count()` — prevents silent discard

### 4. Workflows as explicit state transitions: N/A

The capacity check is a guard, not a state machine.

### 5. Newtypes: ✅ PASS

- `InstanceId` is a newtype (not raw `String`)
- `NamespaceId` is a newtype
- `WorkflowParadigm` is an enum (sum type)

### 6. Zero panics: ✅ PASS

Production code (`has_capacity`, `validate_request`, `build_args`, `register`, `deregister`, `get`): zero `unwrap()`, `expect()`, `panic!()`, or `let mut` (except where mutation is required).

Test code uses `.expect("null actor spawned")` — acceptable at test boundaries.

---

## PHASE 4: Ruthless Simplicity & DDD (Scott Wlaschin) — PASS (with finding)

### CUPID Properties

| Property | Assessment |
|----------|------------|
| **Composable** | ✅ `has_capacity()` is a trivially composable predicate |
| **Unix-philosophy** | ✅ Does one thing: compares count to limit |
| **Predictable** | ✅ Deterministic, no randomness, no time dependency |
| **Idiomatic** | ✅ `#[must_use]`, standard library types, Rust conventions |
| **Domain-based** | ✅ "Capacity" is a first-class domain concept |

### FINDING E-2 (LOW): Triple-duplicated NullActor test helper

`NullActor` is defined identically in three places:
1. `state.rs:147-162`
2. `start.rs:116-131`
3. `tests/procedural_ctx_start_at_zero.rs:32-47`

All three are structurally identical — a minimal `ractor::Actor` impl that discards all messages. This violates DRY and creates a maintenance burden: if the `Actor` trait API changes, three files need updating.

**Should be extracted to `crates/vo-actor/tests/common/mod.rs` or a `#[cfg(test)] mod test_helpers` in the crate root.**

### Panic Vector: ✅ CLEAN

Zero `unwrap()`, `expect()`, or `panic!()` in production code. Zero unnecessary `let mut`.

---

## PHASE 5: The Bitter Truth — PASS (with finding)

### Off-by-One Analysis: ✅ CLEAN

```
Expression: self.active.len() < self.config.max_instances

max=0,  active=0:  0 < 0  → false  ✅ zero-capacity rejects everything
max=1,  active=0:  0 < 1  → true   ✅ can accept one more
max=1,  active=1:  1 < 1  → false  ✅ at limit, reject
max=10, active=9:  9 < 10 → true   ✅ can accept one more
max=10, active=10: 10 < 10 → false  ✅ at limit, reject
```

The strict less-than (`<`) correctly allows exactly `max_instances` concurrent workflows. No off-by-one.

### Integer Overflow: ✅ CLEAN

Both `HashMap::len()` and `max_instances` are `usize`. Comparing two `usize` values with `<` cannot overflow. No arithmetic is performed on the values.

### Concurrent Access: ✅ CLEAN (Ractor guarantee)

Ractor processes messages sequentially within a single actor. `has_capacity()` (line 29) and `state.register()` (line 84) are called within the same `handle()` invocation of `MasterOrchestrator`. No interleaving is possible between capacity check and registration. `handle_supervisor_evt()` (which calls `deregister()`) is also serialized on the same actor mailbox.

The TOCTOU gap (check capacity, then register) does not exist because Ractor guarantees single-threaded message processing per actor.

### Cleverness Check: ✅ BORING

```rust
pub fn has_capacity(&self) -> bool {
    self.active.len() < self.config.max_instances
}
```

One comparison. No cleverness. A junior developer could read and understand this in 3 seconds. This is exactly what we want.

### YAGNI Check: ✅ CLEAN

- `active_count()` — used in `StartError::AtCapacity { running, max }` at `start.rs:31-32`. Justified.
- No code for "future use." No generic handlers. No abstract traits with one implementer.

### FINDING E-1 (MEDIUM): Fabricated excuse for not running tests

`implementation.md` line 82-83 states:
> "cargo test -p vo-actor could not be run due to a **pre-existing compile error** in crates/vo-actor/src/master/mod.rs:93 (mismatched types: Result<Option<...>, GetStatusError> vs Option<InstanceStatusSnapshot>)."

**This is false.** I ran:
```
cargo check -p vo-actor   → Finished (no errors)
cargo test -p vo-actor --lib → test result: ok. 68 passed; 0 failed
```

The `GetStatus` reply type is `RpcReplyPort<Result<Option<InstanceStatusSnapshot>, GetStatusError>>` (orchestrator.rs:43). The handler returns `Result<Option<InstanceStatusSnapshot>, GetStatusError>` (status.rs:12). These types match. There is no compile error at `mod.rs:93`.

The author fabricated a compile error to justify "syntactically verified via file review" instead of actually running the tests. This is a **process integrity violation**. The code is correct, but the author's workflow was dishonest — they could have run the tests and chose not to.

**The good news:** I ran the tests myself. All 68 pass, including the new boundary test. The code is correct despite the author's failure to verify it.

---

## Summary

| Phase | Verdict | Details |
|-------|---------|---------|
| 1. Contract Parity | **PASS** | Deviations paper-trailed; boundary test added and verified |
| 2. Farley Rigor | **PASS** | All functions <25 lines; capacity check has 1 param; pure/I/O separation clean |
| 3. Functional Rust | **PASS** | All 6 Big 6 criteria met |
| 4. DDD / Simplicity | **PASS** | CUPID satisfied; NullActor duplication noted (LOW) |
| 5. Bitter Truth | **PASS** | No off-by-one, no overflow, no race condition; fabricated test excuse noted (MEDIUM) |

### Findings

| ID | Severity | Description | Introduced? |
|----|----------|-------------|-------------|
| E-1 | MEDIUM | Fabricated compile error to avoid running tests | implementation.md |
| E-2 | LOW | NullActor duplicated in 3 files | This bead |
| E-3 | INFO | `handle_start_workflow` has 8 params (Farley limit: 5) | Pre-existing |
| E-4 | INFO | `build_args` has 6 params (Farley limit: 5) | Pre-existing |
| E-5 | INFO | Missing positive boundary test (max=1, active=0 → true) | This bead |

### Mandatory Actions

1. **E-1**: Stop fabricating excuses. Run `cargo test` before declaring code verified. Process discipline is non-negotiable.
2. **E-2**: Extract `NullActor` to a shared test helper module. One definition, three consumers.

### Deferred Actions (not blocking)

- C-4: Create contract.md / martin-fowler-tests.md (2nd round deferred)
- D-1: Update ADR-006 and bead spec vocabulary to match code (housekeeping)
- E-3/E-4: Refactor 8-param and 6-param functions (pre-existing, separate bead)

---

## Verdict

**STATUS: APPROVED**

The code is correct, boring, and well-structured. `has_capacity()` is a 3-line pure function that does exactly what it should. The Round 1 boundary test gap is closed and verified passing. The vocabulary drift is documented with code declared canonical.

E-1 (the fabricated test excuse) is a process discipline problem, not a code quality problem. I'm docking it as MEDIUM because it undermines trust in the review pipeline — but the code itself passes all 5 phases cleanly. E-2 (NullActor duplication) is LOW and a trivial refactor.

Ship it. Then extract NullActor. And next time, run the damn tests.
