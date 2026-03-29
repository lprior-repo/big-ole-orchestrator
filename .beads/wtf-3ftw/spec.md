# Bead: vo-3ftw

## Title
fsm: Parse graph_raw into FsmDefinition

## effort_estimate
2hr

---

## Section 0: Clarifications

- **clarification_status**: closed
- **resolved_clarifications**:
  - The `graph_raw` field on `vo_common::WorkflowDefinition` is a JSON string containing the FSM graph. A schema must be defined for its structure. Chosen schema:
    ```json
    {
      "initial_state": "Pending",
      "transitions": [
        { "from": "Pending", "event": "Authorize", "to": "Authorized", "effects": [] },
        { "from": "Authorized", "event": "Charge", "to": "Charged", "effects": [
          { "effect_type": "CallPayment", "payload": "" }
        ]}
      ],
      "terminal_states": ["Fulfilled", "Failed"]
    }
    ```
  - The parser is a pure function: `parse_fsm(graph_raw: &str) -> Result<FsmDefinition, ParseFsmError>`.
  - It lives in `crates/vo-actor/src/fsm/definition.rs` alongside the `FsmDefinition` struct.
  - `EffectDeclaration` fields: `effect_type: String`, `payload: Bytes`. The JSON `payload` is a string (base64 or raw UTF-8) — deserialized via `Bytes::from(payload_str.as_bytes())`.
  - `terminal_states` is optional in the JSON — if missing, the set is empty.
  - `initial_state` is optional — if missing, no initial state is enforced at parse time (the FSM actor sets its own initial state from instance arguments).
- **assumptions**:
  - `graph_raw` is always valid JSON (caller validates before calling; malformed JSON is a `ParseFsmError::InvalidJson`).
  - Duplicate `(from, event)` pairs in transitions are not validated — last one wins (matches `HashMap::insert` semantics).
  - Effects payloads in `graph_raw` are UTF-8 strings (not base64-encoded).

---

## Section 1: EARS Requirements

### Ubiquitous
- THE SYSTEM SHALL provide a pure function `parse_fsm` that converts a `graph_raw` JSON string into an `FsmDefinition` struct.

### Event-Driven
- WHEN `graph_raw` is a valid JSON object with a `"transitions"` array, THE SYSTEM SHALL populate `FsmDefinition.transitions` with one entry per transition element, keyed by `(from, event)`.
- WHEN `graph_raw` contains a `"terminal_states"` array, THE SYSTEM SHALL populate `FsmDefinition.terminal_states` with those state names.
- WHEN `graph_raw` lacks `"terminal_states"`, THE SYSTEM SHALL produce an `FsmDefinition` with an empty terminal set.

### Unwanted Behaviour
- IF `graph_raw` is not valid JSON, THE SYSTEM SHALL return `ParseFsmError::InvalidJson`.
- IF a transition element is missing `"from"`, `"event"`, or `"to"` fields, THE SYSTEM SHALL return `ParseFsmError::MissingField`.
- IF a transition element contains invalid effects (missing `"effect_type"`), THE SYSTEM SHALL return `ParseFsmError::InvalidEffect`.

---

## Section 2: KIRK Contracts

### Preconditions
- **P-VALID-JSON**: `graph_raw` is a `&str` that may or may not be valid JSON.
- **required_inputs**:
  - `graph_raw: &str` — JSON string from `WorkflowDefinition.graph_raw`

### Postconditions
- **P-DEFINITION-BUILT**: Returns `Ok(FsmDefinition)` with all transitions and terminal states populated.
- **P-ERROR-ON-BAD-INPUT**: Returns `Err(ParseFsmError)` if `graph_raw` is malformed or missing required fields.

### Invariants
- **I-TRANSITION-KEY-UNIQUENESS**: Each `(from, event)` pair maps to at most one `(to, effects)` tuple (enforced by `HashMap::insert`).
- **I-PURE-FUNCTION**: `parse_fsm` performs zero I/O — no logging, no network, no state mutation.
- **I-TYPE-SAFETY**: All returned `FsmDefinition` fields are populated from validated JSON values.

---

## Section 2.5: Research Requirements

- **files_to_read**:
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/fsm/definition.rs` — target struct `FsmDefinition`, new `parse_fsm` function home
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/fsm.rs` — re-exports, `plan_fsm_signal` usage of `FsmDefinition`
  - `/home/lewis/src/vo-engine/crates/vo-common/src/types/workflow.rs` — `WorkflowDefinition` with `graph_raw` field
  - `/home/lewis/src/vo-engine/crates/vo-common/src/events/types.rs` — `EffectDeclaration` struct
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/fsm/tests.rs` — existing FSM test patterns
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/master/state.rs:106` — where `workflow_definition` is passed to `InstanceArguments`
- **research_questions**:
  - Is `serde_json` already a dependency of `vo-actor`? (Check `Cargo.toml`.)
  - Are there any existing JSON parsing patterns in the `vo-actor` crate to follow?

---

## Section 3: Inversions

### Data Integrity Failures
- **INVALID-JSON**: `graph_raw` is `"not json{"` → `serde_json::from_str` fails → return `ParseFsmError::InvalidJson(String)`.
- **MISSING-TRANSITIONS**: `graph_raw` is `{"states":[]}` with no `"transitions"` key → return `ParseFsmError::MissingField("transitions")`.
- **BAD-TRANSITION-ELEMENT**: A transition object lacks `"from"` → return `ParseFsmError::MissingField("from")`.
- **BAD-EFFECT**: An effect object lacks `"effect_type"` → return `ParseFsmError::InvalidEffect("missing effect_type")`.

### Schema Failures
- **EXTRA-FIELDS**: `graph_raw` contains unknown fields → silently ignored (forward-compatible).
- **EMPTY-TRANSITIONS**: `"transitions": []` → valid, returns `FsmDefinition` with empty transition map.
- **NULL-TERMINAL-STATES**: `"terminal_states": null` → treated as empty set.

---

## Section 4: ATDD Acceptance Tests

### Happy Paths

**HP-1: Parse single transition**
- **real_input**: `r#"{"transitions":[{"from":"Pending","event":"Authorize","to":"Authorized","effects":[]}]}"#`
- **expected_output**: `FsmDefinition` with `transitions` containing key `("Pending", "Authorize")` mapping to `("Authorized", [])`. `terminal_states` is empty.

**HP-2: Parse transitions with effects**
- **real_input**: `r#"{"transitions":[{"from":"Pending","event":"Charge","to":"Charged","effects":[{"effect_type":"CallPayment","payload":""}]}]}"#`
- **expected_output**: Transition key `("Pending", "Charge")` → `("Charged", [EffectDeclaration { effect_type: "CallPayment", payload: Bytes::new() }])`.

**HP-3: Parse with terminal states**
- **real_input**: `r#"{"transitions":[],"terminal_states":["Fulfilled","Failed"]}"#`
- **expected_output**: `FsmDefinition` with `terminal_states` = `{"Fulfilled", "Failed"}`. `is_terminal("Fulfilled")` returns `true`. `is_terminal("Pending")` returns `false`.

**HP-4: Parse without terminal_states field**
- **real_input**: `r#"{"transitions":[]}"#`
- **expected_output**: `FsmDefinition` with empty `terminal_states`. No error.

**HP-5: Multiple transitions building a workflow**
- **real_input**:
  ```json
  {"transitions":[
    {"from":"Pending","event":"Authorize","to":"Authorized","effects":[]},
    {"from":"Authorized","event":"Charge","to":"Charged","effects":[]},
    {"from":"Charged","event":"Fulfill","to":"Fulfilled","effects":[]}
  ],"terminal_states":["Fulfilled"]}
  ```
- **expected_output**: Three transitions. `is_terminal("Fulfilled")` is `true`. `transition("Pending", "Authorize")` returns `Some(("Authorized", []))`.

### Error Paths

**EP-1: Invalid JSON**
- **real_input**: `"not json{{{"`
- **expected_output**: `Err(ParseFsmError::InvalidJson(_))`

**EP-2: Missing transitions field**
- **real_input**: `"{}"`
- **expected_output**: `Err(ParseFsmError::MissingField("transitions"))`

**EP-3: Transition missing 'from' field**
- **real_input**: `r#"{"transitions":[{"event":"Go","to":"Done","effects":[]}]}"#`
- **expected_output**: `Err(ParseFsmError::MissingField("from"))`

**EP-4: Effect missing 'effect_type' field**
- **real_input**: `r#"{"transitions":[{"from":"A","event":"B","to":"C","effects":[{"payload":""}]}]}"#`
- **expected_output**: `Err(ParseFsmError::InvalidEffect(_))`

---

## Section 5: E2E Tests

### Pipeline Test: `parse_fsm_roundtrip_with_plan_fsm_signal`

**Setup**:
1. Construct a `graph_raw` JSON string with transitions `Pending→Authorize→Authorized` and `Authorized→Charge→Charged`, terminal state `"Charged"`.
2. Call `parse_fsm(&graph_raw)`.

**Execute**:
- Call `def.transition("Pending", "Authorize")`.
- Call `def.is_terminal("Charged")`.
- Build an `FsmActorState::new("Pending")` and call `plan_fsm_signal(&def, &state, "Authorize")`.

**Verify**:
- `parse_fsm` returns `Ok(def)`.
- `def.transition("Pending", "Authorize")` returns `Some(("Authorized", []))`.
- `def.is_terminal("Charged")` returns `true`.
- `plan_fsm_signal` returns `Some(FsmTransitionPlan { next_state.current_state: "Authorized", .. })`.

**Cleanup**: None (pure functions, no state).

---

## Section 5.5: Verification Checkpoints

- **Gate 0 (compile)**: `cargo check -p vo-actor` passes.
- **Gate 1 (clippy)**: `cargo clippy -p vo-actor -- -D warnings` passes.
- **Gate 2 (unit tests)**: `cargo test -p vo-actor -- fsm::definition` passes — all HP and EP tests green.
- **Gate 3 (workspace)**: `cargo test --workspace` passes — no regressions.

---

## Section 6: Implementation Tasks

### Phase 0: Define JSON schema types and error enum
- [ ] Add `serde` intermediate structs (`FsmGraph`, `FsmTransitionJson`, `FsmEffectJson`) in `crates/vo-actor/src/fsm/definition.rs`
- [ ] Add `ParseFsmError` enum with variants: `InvalidJson(String)`, `MissingField(&'static str)`, `InvalidEffect(String)`
- [ ] Implement `std::fmt::Display` and `std::error::Error` for `ParseFsmError`
- **parallelization**: none

### Phase 1: Implement `parse_fsm` function
- [ ] Add `pub fn parse_fsm(graph_raw: &str) -> Result<FsmDefinition, ParseFsmError>` in `crates/vo-actor/src/fsm/definition.rs`
- [ ] Deserialize `graph_raw` into `FsmGraph` using `serde_json::from_str`
- [ ] Iterate transitions, validate required fields (`from`, `event`, `to`)
- [ ] Convert each effect JSON to `EffectDeclaration { effect_type, payload: Bytes::from(payload_str) }`
- [ ] Populate `FsmDefinition` via `add_transition` and `add_terminal_state`
- [ ] Re-export `parse_fsm` and `ParseFsmError` from `crates/vo-actor/src/fsm.rs`
- **parallelization**: none

### Phase 2: Unit tests
- [ ] HP-1 through HP-5 in `#[cfg(test)] mod tests` inside `definition.rs`
- [ ] EP-1 through EP-4 error path tests
- [ ] E2E roundtrip test with `plan_fsm_signal`
- **parallelization**: none

### Phase 3: Quality gates
- [ ] `cargo check --workspace`
- [ ] `cargo clippy --workspace -- -D warnings`
- [ ] `cargo test --workspace`

---

## Section 7: Failure Modes

| Symptom | Cause | Fix |
|---|---|---|
| `parse_fsm` returns `Ok` but `FsmDefinition` has zero transitions | `serde_json` silently defaulting missing `"transitions"` | Explicitly check `"transitions"` exists in JSON before deserializing, or use `#[serde(deny_unknown_fields)]` on `FsmGraph` with `transitions` as required |
| Clippy warns about `HashMap` key type | Using `(String, String)` tuple as key — this is correct and matches `definition.rs:7` | No fix needed, existing pattern |
| `Bytes` not `Serialize`/`Deserialize` for intermediate struct | `EffectDeclaration.payload` is `Bytes` which needs custom serde handling | Keep intermediate `FsmEffectJson` with `payload: String`, convert to `Bytes` in `parse_fsm` |
| Test fails: effect payload mismatch | `Bytes::from("")` vs `Bytes::new()` — both are empty, should be equal | Verify with `.as_ref()` comparison |

---

## Section 7.5: Anti-Hallucination

- **read_before_write**:
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/fsm/definition.rs` — before adding `parse_fsm` and `ParseFsmError`
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/fsm.rs` — before adding re-exports
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/fsm/tests.rs` — verify existing test patterns
  - `/home/lewis/src/vo-engine/crates/vo-common/src/events/types.rs` — verify `EffectDeclaration` fields
  - `/home/lewis/src/vo-engine/crates/vo-actor/Cargo.toml` — verify `serde_json` dependency exists
- **no_invent**:
  - Do NOT add fields to `FsmDefinition` — it has exactly `transitions: HashMap<(String, String), (String, Vec<EffectDeclaration>)>` and `terminal_states: HashSet<String>`
  - Do NOT add methods to `FsmDefinition` that already exist (`add_transition`, `add_terminal_state`, `is_terminal`, `transition`)
  - Do NOT change the visibility of `FsmDefinition` fields (currently private)

---

## Section 7.6: Context Survival

- **progress_file**: `.beads/vo-3ftw/progress.md`
- **recovery_instructions**:
  1. Read this spec and all files in `read_before_write`.
  2. Check `progress.md` for last completed phase.
  3. Resume from that phase.
  4. If `progress.md` missing, start at Phase 0.

---

## Section 8: Completion Checklist

- [ ] `ParseFsmError` enum with `InvalidJson`, `MissingField`, `InvalidEffect` variants
- [ ] `parse_fsm(graph_raw: &str) -> Result<FsmDefinition, ParseFsmError>` implemented
- [ ] `serde_json` intermediate structs for deserialization
- [ ] `parse_fsm` and `ParseFsmError` re-exported from `vo_actor::fsm`
- [ ] HP-1 through HP-5 tests pass
- [ ] EP-1 through EP-4 error path tests pass
- [ ] E2E roundtrip test with `plan_fsm_signal` passes
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` passes

---

## Section 9: Context

- **related_files**:
  - `crates/vo-actor/src/fsm/definition.rs` — `FsmDefinition` struct, new `parse_fsm` home (PRIMARY, MODIFY)
  - `crates/vo-actor/src/fsm.rs` — re-exports (MODIFY)
  - `crates/vo-common/src/types/workflow.rs` — `WorkflowDefinition.graph_raw` (READ)
  - `crates/vo-common/src/events/types.rs` — `EffectDeclaration { effect_type, payload }` (READ)
  - `crates/vo-actor/src/fsm/tests.rs` — existing test patterns (READ)
  - `crates/vo-actor/Cargo.toml` — verify `serde_json` dep (READ)
  - `crates/vo-actor/src/master/state.rs:106` — downstream consumer of `WorkflowDefinition` (READ)

---

## Section 10: AI Hints

- **do**:
  - Use intermediate `serde` structs (`FsmGraph`, `FsmTransitionJson`, `FsmEffectJson`) with `#[derive(Deserialize)]` — keep them private to `definition.rs`
  - Use `Bytes::from(payload_str.as_bytes())` for effect payloads (the JSON `payload` field is a UTF-8 string)
  - Use `thiserror` for `ParseFsmError` — it's already in `vo-actor` dependencies (see `types.rs:28`)
  - Put unit tests in a `#[cfg(test)] mod tests` block inside `definition.rs` — matches the pattern in `state.rs`
  - Re-export from `fsm.rs` via `pub use definition::{parse_fsm, ParseFsmError}`
- **do_not**:
  - Do NOT add `Serialize`/`Deserialize` to `FsmDefinition` — it is a runtime struct, not a wire format
  - Do NOT make `FsmDefinition` fields public
  - Do NOT add async — this is a pure synchronous function
  - Do NOT log inside `parse_fsm` — pure function, no side effects
  - Do NOT use `unwrap()` or `expect()` — return `ParseFsmError` for all failure cases
