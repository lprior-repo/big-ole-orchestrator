# Bead: vo-2nty

## Title
serve: Load definitions from KV into registry on startup

## effort_estimate
30min

---

## Section 0: Clarifications

- **clarification_status**: closed
- **resolved_clarifications**:
  - The `vo-definitions` KV bucket stores definitions as JSON-serialized `WorkflowDefinition` (with key format `<ns>/<workflow_type>`). On startup, `serve.rs` must scan all keys and register each into the in-memory `WorkflowRegistry`.
  - The registry lives inside `OrchestratorState` which is owned by the `MasterOrchestrator` actor. The load must happen BEFORE the orchestrator starts (or via a new actor message). Decision: load into registry before `MasterOrchestrator::spawn`, populating `OrchestratorConfig` with a pre-seeded registry, OR add a `definitions` field to `OrchestratorConfig`. Chosen approach: add `pub definitions: Vec<(String, WorkflowDefinition)>` to `OrchestratorConfig` and consume them in `OrchestratorState::new()` or `MasterOrchestrator::pre_start()`.
  - KV scan uses `Store::keys()` + `Store::get()` — same pattern as `vo-worker/src/timer/loop.rs:128-143`.
  - Definition values are JSON-serialized `WorkflowDefinition` (from `vo_common::WorkflowDefinition`).
- **assumptions**:
  - The `vo-definitions` bucket is already provisioned by `provision_kv_buckets()` before this code runs.
  - KV values are JSON-encoded `WorkflowDefinition` structs (not MessagePack).
  - Malformed entries are logged and skipped (no abort).
  - Empty bucket is a valid state (no definitions loaded).

---

## Section 1: EARS Requirements

### Ubiquitous
- THE SYSTEM SHALL load all workflow definitions from the `vo-definitions` NATS KV bucket into the `WorkflowRegistry` before accepting any workflow start requests.

### Event-Driven
- WHEN `wtf serve` starts and KV buckets are provisioned, THE SYSTEM SHALL scan every key in the `vo-definitions` bucket and deserialize each value as `WorkflowDefinition`.
- WHEN a definition key fails to deserialize, THE SYSTEM SHALL log a warning with the key name and continue loading remaining definitions.

### Unwanted Behaviour
- IF the `vo-definitions` bucket is empty, THE SYSTEM SHALL proceed with an empty registry and log an info-level message "No workflow definitions found in KV".
- IF the `vo-definitions` bucket contains a key with invalid JSON, THE SYSTEM SHALL NOT abort startup.

---

## Section 2: KIRK Contracts

### Preconditions
- **P-KV-READY**: `provision_kv_buckets()` has returned `Ok(KvStores)` containing a valid `definitions: Store`.
- **required_inputs**:
  - `kv.definitions` — a `Store` handle for the `vo-definitions` bucket
  - NATS connection is alive and KV bucket is reachable

### Postconditions
- **P-REGISTRY-LOADED**: `WorkflowRegistry` in `OrchestratorState` contains one entry per valid key in `vo-definitions`.
- **P-COUNT-LOGGED**: A log line states how many definitions were loaded (e.g. `"Loaded N workflow definitions from KV"`).

### Invariants
- **I-NO-DUPS**: Each KV key maps to exactly one registry entry (last-write-wins is inherent to KV).
- **I-TYPE-SAFETY**: Only `WorkflowDefinition` structs (deserialized from JSON) are inserted into the registry.
- **I-ORDER-INDEPENDENT**: The order in which definitions are loaded MUST NOT affect the final registry state.

---

## Section 2.5: Research Requirements

- **files_to_read**:
  - `/home/lewis/src/vo-engine/crates/vo-cli/src/commands/serve.rs` — insertion point
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/master/state.rs` — `OrchestratorConfig`, `OrchestratorState`
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/master/registry.rs` — `WorkflowRegistry::register_definition`
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/master/mod.rs` — `MasterOrchestrator::pre_start`
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/messages/orchestrator.rs` — `OrchestratorMsg` enum
  - `/home/lewis/src/vo-engine/crates/vo-common/src/types/workflow.rs` — `WorkflowDefinition` struct
  - `/home/lewis/src/vo-engine/crates/vo-storage/src/kv.rs` — `Store`, `definition_key`
  - `/home/lewis/src/vo-engine/crates/vo-worker/src/timer/loop.rs` — KV key scan pattern (`Store::keys()`)
- **research_questions**:
  - What serialization format is used for definition values in KV? (Assume JSON from `serde_json` — verify by checking any existing write path.)
  - Is there an existing write path that stores definitions into KV, to confirm the serialization format?

---

## Section 3: Inversions

### Data Integrity Failures
- **CORRUPT-JSON**: A KV value contains invalid JSON → `serde_json::from_slice` returns `Err` → log warning, skip key, continue.
- **MISSING-FIELD**: A KV value deserialifies to JSON but lacks required fields (`paradigm`, `graph_raw`) → `serde` reject → log warning, skip.
- **WRONG-TYPE**: A KV value is valid JSON but not a `WorkflowDefinition` (e.g., `InstanceMetadata`) → deserialization fails → log warning, skip.

### Integration Failures
- **KV-UNREACHABLE**: `Store::keys()` or `Store::get()` returns a NATS error → return `Err(anyhow!("failed to load definitions from KV: {e}"))` — fatal to startup.
- **EMPTY-BUCKET**: Zero keys returned → log info, proceed with empty registry.

---

## Section 4: ATDD Acceptance Tests

### Happy Paths

**HP-1: Load single FSM definition**
- **real_input**: KV contains key `"payments/checkout"` with value `{"paradigm":"Fsm","graph_raw":"{\"states\":[\"start\",\"end\"],\"transitions\":[]}","description":"Payment checkout"}`
- **expected_output**: `OrchestratorState.registry.definitions` contains key `"payments/checkout"` with `WorkflowDefinition { paradigm: WorkflowParadigm::Fsm, graph_raw: "...", description: Some("Payment checkout".to_owned()) }`
- Log line: `"Loaded 1 workflow definitions from KV"`

**HP-2: Load multiple definitions across namespaces**
- **real_input**: KV contains `"ns1/wf-a"` and `"ns2/wf-b"` with valid JSON definitions
- **expected_output**: Registry contains both entries. Log: `"Loaded 2 workflow definitions from KV"`

**HP-3: Empty bucket**
- **real_input**: KV bucket exists with zero keys
- **expected_output**: Registry is empty. Log: `"No workflow definitions found in KV"`

### Error Paths

**EP-1: Corrupt JSON in one key**
- **real_input**: KV contains `"ok/wf1"` (valid) and `"bad/wf2"` with value `"not-json{{{"`
- **expected_output**: Registry contains only `"ok/wf1"`. Warning logged for `"bad/wf2"`. Log: `"Loaded 1 workflow definitions from KV"`

**EP-2: Valid JSON but wrong schema (missing paradigm)**
- **real_input**: KV contains key `"x/y"` with value `{"graph_raw":"{}"}`
- **expected_output**: Key skipped, warning logged. Registry empty.

---

## Section 5: E2E Tests

### Pipeline Test: `serve_loads_definitions_on_startup`

**Setup**:
1. Start NATS (Docker `vo-nats-test` on port 4222).
2. Connect and provision KV buckets via `provision_kv_buckets()`.
3. Write test definition: `kv.definitions.put("test-ns/my-wf", serde_json::to_vec(&WorkflowDefinition { paradigm: WorkflowParadigm::Dag, graph_raw: r#"{"nodes":[],"edges":[]}"#.to_owned(), description: Some("test".to_owned()) }).unwrap().into()).await`.
4. Invoke `load_definitions_from_kv(&kv.definitions)`.

**Execute**:
- Call the new function directly (unit-level with live NATS).

**Verify**:
- Returns `Ok(Vec<(String, WorkflowDefinition)>)` with exactly one entry: `("test-ns/my-wf", WorkflowDefinition { paradigm: Dag, ... })`.
- Deserialized paradigm is `WorkflowParadigm::Dag`.

**Cleanup**:
- Purge `vo-definitions` bucket or use unique test key prefix.

---

## Section 5.5: Verification Checkpoints

- **Gate 0 (compile)**: `cargo check --workspace` passes.
- **Gate 1 (clippy)**: `cargo clippy --workspace -- -D warnings` passes.
- **Gate 2 (unit tests)**: `cargo test --workspace` passes (new test in `serve.rs` tests module + any registry tests).
- **Gate 3 (integration)**: New function tested with live NATS — `Store::keys()` + `Store::get()` pattern verified.

---

## Section 6: Implementation Tasks

### Phase 0: Modify `OrchestratorConfig` to accept pre-seeded definitions
- [ ] Add `pub definitions: Vec<(String, WorkflowDefinition)>` field to `OrchestratorState::new()` in `crates/vo-actor/src/master/state.rs`
- [ ] Update `OrchestratorState::new()` to accept and register definitions
- [ ] Update `Default` for `OrchestratorConfig` if needed (empty vec)
- **parallelization**: none

### Phase 1: Create `load_definitions_from_kv` function
- [ ] Add `load_definitions_from_kv(store: &Store) -> anyhow::Result<Vec<(String, WorkflowDefinition)>>` in `crates/vo-cli/src/commands/serve.rs` (or a new `crates/vo-storage/src/definitions.rs`)
- [ ] Use `store.keys().await` → iterate → `store.get(&key).await` → `serde_json::from_slice::<WorkflowDefinition>(&value)`
- [ ] Log count on success, warn on each failure, info on empty
- **parallelization**: none

### Phase 2: Wire into `run_serve`
- [ ] In `run_serve()` after `provision_storage()`, call `load_definitions_from_kv(&kv.definitions)`
- [ ] Pass loaded definitions to `OrchestratorState` construction
- [ ] May require changing `MasterOrchestrator::pre_start` or `OrchestratorState::new` signature to consume definitions
- **parallelization**: none

### Phase 3: Tests
- [ ] Unit test for `load_definitions_from_kv` with mock (or integration test with live NATS)
- [ ] Unit test: `OrchestratorState::new()` with pre-seeded definitions populates registry
- [ ] Unit test: corrupt JSON entry is skipped with warning

### Phase 4: Quality gates
- [ ] `cargo check --workspace`
- [ ] `cargo clippy --workspace -- -D warnings`
- [ ] `cargo test --workspace`

---

## Section 7: Failure Modes

| Symptom | Cause | Fix |
|---|---|---|
| Panic on startup | `serde_json::from_slice` result not handled | Use `match` or `if let Ok(...)` — never unwrap |
| Registry empty after startup with valid definitions | `Store::keys()` pagination not awaited fully | Use `while let Some(result) = keys.next().await` loop (same as `timer/loop.rs:128-143`) |
| Duplicate definition overwrites | KV history > 1 and scan returns stale keys | KV `history: 5` on definitions bucket; `Store::keys()` returns latest value per key — this is correct |
| Compile error: `WorkflowDefinition` not in scope | Missing import | Add `use vo_actor::WorkflowDefinition` or `use vo_common::WorkflowDefinition` |

---

## Section 7.5: Anti-Hallucination

- **read_before_write**:
  - `/home/lewis/src/vo-engine/crates/vo-cli/src/commands/serve.rs` — before adding load call
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/master/state.rs` — before modifying `OrchestratorState`
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/master/registry.rs` — verify `register_definition` signature
  - `/home/lewis/src/vo-engine/crates/vo-actor/src/master/mod.rs` — before changing `pre_start`
  - `/home/lewis/src/vo-engine/crates/vo-common/src/types/workflow.rs` — verify `WorkflowDefinition` fields
  - `/home/lewis/src/vo-engine/crates/vo-storage/src/kv.rs` — verify `Store` import path
  - `/home/lewis/src/vo-engine/crates/vo-worker/src/timer/loop.rs` — verify `keys()` scan pattern

---

## Section 7.6: Context Survival

- **progress_file**: `.beads/vo-2nty/progress.md`
- **recovery_instructions**:
  1. Read this spec and all files in `read_before_write`.
  2. Check `progress.md` for last completed phase.
  3. Resume from that phase.
  4. If `progress.md` missing, start at Phase 0.

---

## Section 8: Completion Checklist

- [ ] `OrchestratorState` accepts pre-seeded definitions
- [ ] `load_definitions_from_kv` function implemented with KV scan pattern
- [ ] `run_serve` calls load and passes definitions to orchestrator
- [ ] Empty bucket logs info message
- [ ] Corrupt entries log warnings and are skipped
- [ ] Unit tests pass
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` passes

---

## Section 9: Context

- **related_files**:
  - `crates/vo-cli/src/commands/serve.rs` — insertion point for load call (PRIMARY)
  - `crates/vo-actor/src/master/state.rs` — `OrchestratorConfig`, `OrchestratorState` (MODIFY)
  - `crates/vo-actor/src/master/registry.rs` — `WorkflowRegistry::register_definition` (READ)
  - `crates/vo-actor/src/master/mod.rs` — `MasterOrchestrator::pre_start` (MODIFY)
  - `crates/vo-common/src/types/workflow.rs` — `WorkflowDefinition` (READ)
  - `crates/vo-storage/src/kv.rs` — `Store`, `KvStores`, `definition_key` (READ)
  - `crates/vo-worker/src/timer/loop.rs` — KV scan pattern reference (READ)
  - `crates/vo-actor/src/messages/orchestrator.rs` — `OrchestratorMsg` enum (READ, may not need changes)

---

## Section 10: AI Hints

- **do**:
  - Use `Store::keys().await` → `while let Some(key_result) = keys.next().await` pattern from `timer/loop.rs:128-143`
  - Deserialize with `serde_json::from_slice::<WorkflowDefinition>(&value)`
  - Use `tracing::warn!` for skipped entries, `tracing::info!` for count
  - Register via `registry.register_definition(&workflow_type, definition)` — the key in KV is `<ns>/<workflow_type>`, extract workflow_type by splitting on `/`
  - Add the `definitions` vec to `OrchestratorState::new()` or pass through config, NOT via actor message
- **do_not**:
  - Do NOT add a new `OrchestratorMsg` variant for this — it's a startup-only operation
  - Do NOT use `unwrap()` or `expect()` on deserialization results
  - Do NOT abort startup if individual entries fail to deserialize
  - Do NOT modify the `provision_kv_buckets` function
  - Do NOT use MessagePack — definitions are JSON
