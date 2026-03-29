# vo-5fow

## 0 Clarifications

| # | Question | Answer |
|---|----------|--------|
| 1 | NATS test container name? | `vo-nats-test` on port 4222 (per AGENTS.md) |
| 2 | Heartbeat TTL? | 10s (`max_age: Duration::from_secs(10)` in `crates/vo-storage/src/kv.rs:101`) |
| 3 | Heartbeat interval? | 5s (`Duration::from_secs(5)` in `crates/vo-actor/src/instance/actor.rs:54`) |
| 4 | Heartbeat KV bucket name? | `vo-heartbeats` (key prefix `hb/<instance_id>`) |
| 5 | Crash simulation method? | Stop the `WorkflowInstance` actor via `ActorRef::stop()` — triggers `post_stop`, aborts tasks, and deregisters from `OrchestratorState.active` via supervision |
| 6 | Which paradigm for E2E? | FSM — simplest to verify state reconstruction without procedural checkpoint complexity |
| 7 | Snapshot strategy? | ADR-019 sled snapshots at `SNAPSHOT_INTERVAL = 100` events (stub currently). Recovery may do full replay. Test must work with or without snapshots. |
| 8 | Recovery trigger path? | `run_heartbeat_watcher` → `process_heartbeat_entry` (Delete/Purge) → `OrchestratorMsg::HeartbeatExpired` → `handle_heartbeat_expired` → `attempt_recovery` → `WorkflowInstance::spawn_linked` |

---

## 1 EARS

| ID | Requirement |
|----|-------------|
| E1 | **WHEN** a `WorkflowInstance` actor stops (crash) **THEN** its heartbeat entry in `vo-heartbeats` KV **SHALL** expire within 10s due to `max_age` TTL |
| E2 | **WHILE** the heartbeat watcher is running, **IF** a heartbeat entry is deleted/purged by TTL, **THEN** the watcher **SHALL** send `OrchestratorMsg::HeartbeatExpired { instance_id }` to the `MasterOrchestrator` |
| E3 | **WHEN** `HeartbeatExpired` arrives and the instance is NOT in `OrchestratorState.active`, **THEN** `handle_heartbeat_expired` **SHALL** fetch `InstanceMetadata` from the state store and spawn a new `WorkflowInstance` via `spawn_linked` |
| E4 | **AFTER** the recovered instance spawns, it **SHALL** load initial state via `load_initial_state` (snapshot + replay) and reach `InstancePhase::Live` |
| E5 | **WHEN** the recovered instance reaches `Live` phase, its paradigm state **SHALL** match the pre-crash state (same FSM `current_state`, same `total_events_applied`) |
| E6 | **IF** `check_recovery_preconditions` finds the instance still in `active`, recovery **SHALL** be skipped (no duplicate spawn) |

---

## 2 KIRK Contracts

### Contract C1: HeartbeatWatcherDetectsExpiry

```rust
// File: crates/vo-actor/src/heartbeat.rs
// Function: process_heartbeat_entry

// PRE: entry.operation == Delete | Purge, key starts with "hb/"
// POST: orchestrator.cast(OrchestratorMsg::HeartbeatExpired { instance_id }) called exactly once
// INVARIANT: No HeartbeatExpired sent for non-hb/ keys
```

### Contract C2: RecoverySkipsActiveInstance

```rust
// File: crates/vo-actor/src/master/handlers/heartbeat.rs
// Function: check_recovery_preconditions

// PRE: state.active.contains_key(&instance_id) == true
// POST: returns None
// INVARIANT: state.active is not modified
```

### Contract C3: RecoverySpawnsFromMetadata

```rust
// File: crates/vo-actor/src/master/handlers/heartbeat.rs
// Function: attempt_recovery

// PRE: state.config.state_store is Some with valid InstanceMetadata
// POST: WorkflowInstance::spawn_linked called with name "wf-recovered-{instance_id}"
// POST: state.active contains new ActorRef for instance_id
// POST: in_flight_guard key removed after attempt (success or failure)
```

### Contract C4: RecoveredInstanceReplaysCorrectly

```rust
// File: crates/vo-actor/src/instance/init.rs
// Functions: load_initial_state, replay_events

// PRE: snapshot_db has SnapshotRecord{seq: N}, JetStream has events 1..=M where M > N
// POST: from_seq == N + 1
// POST: events N+1..=M replayed and applied to paradigm_state
// POST: total_events_applied == M
// INVARIANT: paradigm_state matches original (deterministic replay)
```

### Contract C5: HeartbeatWrittenEvery5s

```rust
// File: crates/vo-actor/src/instance/actor.rs (pre_start) + instance/handlers.rs (handle_heartbeat)
// Trigger: send_interval(Duration::from_secs(5), || InstanceMsg::Heartbeat)

// PRE: state.args.state_store is Some
// POST: store.put_heartbeat(engine_node_id, instance_id) called
// INVARIANT: Key format "hb/{instance_id}" in vo-heartbeats bucket
```

---

## 2.5 Research

### Key Source Files

| File | Lines | Role |
|------|-------|------|
| `crates/vo-actor/src/heartbeat.rs` | 167 | Heartbeat expiry watcher — watches KV for Delete/Purge ops |
| `crates/vo-actor/src/master/handlers/heartbeat.rs` | 99 | Recovery handler — fetches metadata, spawns recovered instance |
| `crates/vo-actor/src/master/mod.rs` | 149 | `MasterOrchestrator` actor — dispatches `HeartbeatExpired` to handler |
| `crates/vo-actor/src/master/state.rs` | 219 | `OrchestratorState.active` map — deregistered on child termination |
| `crates/vo-actor/src/instance/actor.rs` | 87 | `WorkflowInstance` — heartbeat timer at 5s, aborts tasks in `post_stop` |
| `crates/vo-actor/src/instance/init.rs` | 140 | `load_initial_state` + `replay_events` — snapshot + JetStream replay |
| `crates/vo-actor/src/instance/handlers.rs` | 222 | `handle_heartbeat` — writes to `vo-heartbeats` KV |
| `crates/vo-actor/src/instance/state.rs` | 69 | `InstanceState` — `paradigm_state`, `total_events_applied` |
| `crates/vo-storage/src/kv.rs` | 209 | KV provisioning — `vo-heartbeats` with `max_age: 10s`, `heartbeat_key()` |
| `crates/vo-storage/src/snapshots.rs` | 258 | sled snapshots — `read_snapshot`, `write_snapshot`, CRC32 validation |
| `crates/vo-storage/src/replay.rs` | 255 | `ReplayConsumer` — ephemeral push consumer from `from_seq` |
| `crates/vo-storage/src/provision.rs` | 204 | Stream provisioning — `vo-events` stream |

### Recovery Data Flow

```
WorkflowInstance dies
  → post_stop fires → ActorRef stops responding
  → heartbeat timer stops → no more KV writes for hb/{instance_id}
  → 10s passes → NATS deletes entry (max_age expiry)
  → run_heartbeat_watcher sees Delete op
  → orchestrator.cast(OrchestratorMsg::HeartbeatExpired { instance_id })
  → check_recovery_preconditions: active.contains_key → false (deregistered by supervisor)
  → acquire_in_flight_guard: insert instance_id → true (not already recovering)
  → fetch_metadata: store.get_instance_metadata → Some(InstanceMetadata{...})
  → build_recovery_args: InstanceSeed from metadata → InstanceArguments
  → WorkflowInstance::spawn_linked("wf-recovered-{id}", ...)
  → pre_start: load_initial_state (snapshot + replay) → transition_to_live → Live
```

### Test Infrastructure Gaps

- Existing tests in `crates/vo-actor/tests/` are unit-level (no live NATS, no actor spawning)
- No existing test uses `run_heartbeat_watcher` or `handle_heartbeat_expired`
- `spawn_workflow_test.rs` is the closest to an integration test — examine its NATS setup pattern

---

## 3 Inversions

| # | Failure | Why Invert | Detection |
|---|---------|------------|-----------|
| I1 | Watcher misses Delete event | Race between KV expiry and watch_all() | Test publishes events, kills actor, asserts watcher receives `HeartbeatExpired` within 15s timeout |
| I2 | Recovery spawns duplicate instance | In-flight guard failure | Assert `OrchestratorState.active_count()` is exactly 1 after recovery |
| I3 | Recovered instance has wrong state | Replay bug or snapshot corruption | Assert `total_events_applied` and `paradigm_state` match pre-crash |
| I4 | Heartbeat not expired | Actor stop doesn't deregister from active | Assert `OrchestratorState.active` is empty after kill (via supervision event) |
| I5 | Recovery skipped because active not cleared | Supervision event race | Add small sleep (200ms) after kill to allow supervisor to process `ActorTerminated` |

---

## 4 ATDD Tests

### T1: `heartbeat_key_parsed_correctly_for_recovery`

```rust
// File: crates/vo-actor/tests/heartbeat_expiry_recovery.rs
#[test]
fn heartbeat_key_parsed_correctly_for_recovery() {
    use vo_actor::heartbeat::instance_id_from_heartbeat_key;
    let key = "hb/crash-test-inst-001";
    let id = instance_id_from_heartbeat_key(key);
    assert!(id.is_some());
    assert_eq!(id.unwrap().as_str(), "crash-test-inst-001");
}
```

### T2: `recovery_skipped_when_instance_still_active`

```rust
#[test]
fn recovery_preconditions_skip_active_instance() {
    // Verify check_recovery_preconditions logic:
    // If state.active.contains_key(&instance_id), recovery is skipped.
    // Cannot call directly (private), but test the observable behavior:
    // cast HeartbeatExpired while instance is running → no duplicate spawn.
}
```

### T3: `in_flight_guard_prevents_duplicate_recovery`

```rust
#[tokio::test]
async fn duplicate_heartbeat_expired_triggers_single_recovery() {
    // Cast HeartbeatExpired twice for same instance_id
    // Only one recovery attempt should proceed
    // Second should be skipped by in_flight guard
}
```

### T4: `snapshot_plus_replay_produces_correct_state`

```rust
#[test]
fn snapshot_replay_determinism() {
    use vo_actor::fsm::{apply_event, ExecutionPhase, FsmActorState};
    // Build FSM state through events, serialize snapshot, deserialize,
    // replay remaining events, assert final state matches.
}
```

### T5: `recovered_instance_metadata_matches_original`

```rust
#[test]
fn recovery_args_built_from_metadata() {
    use vo_common::{InstanceId, InstanceMetadata, WorkflowParadigm};
    let meta = InstanceMetadata {
        namespace: "test".into(),
        instance_id: InstanceId::new("inst-1"),
        workflow_type: "checkout".into(),
        paradigm: WorkflowParadigm::Fsm,
        engine_node_id: "node-1".into(),
    };
    // build_recovery_args should produce InstanceArguments with matching fields
}
```

---

## 5 E2E Tests

### E2E-1: `crash_recovery_fsm_heartbeat_expiry` (primary)

**Location:** `crates/vo-actor/tests/heartbeat_expiry_recovery.rs`

**Prerequisites:** NATS running (`vo-nats-test` container, port 4222)

**Setup (per test):**
1. Connect to NATS, create JetStream context
2. Provision `vo-events` stream and `vo-heartbeats` KV bucket via `vo_storage::provision`
3. Create temp sled dir for snapshots
4. Build `OrchestratorConfig` with real `event_store`, `state_store`, `snapshot_db`
5. Spawn `MasterOrchestrator` via `Actor::spawn`
6. Start `run_heartbeat_watcher` as background tokio task
7. Register FSM workflow definition in `WorkflowRegistry`

**Test Steps:**
```
GIVEN: MasterOrchestrator running with heartbeat watcher
  AND: FSM workflow "checkout-fsm" registered

WHEN: Start workflow via StartWorkflow { paradigm: Fsm, workflow_type: "checkout-fsm" }
  AND: Publish FSM events (InstanceStarted, TransitionApplied Created→Authorized)
  AND: Capture pre-crash total_events_applied

WHEN: Kill WorkflowInstance actor (myself.stop())
  AND: Wait 200ms for supervisor ActorTerminated to clear OrchestratorState.active

WHEN: Wait up to 15s for heartbeat KV entry to expire (TTL=10s)
  AND: Heartbeat watcher fires HeartbeatExpired

THEN: Recovered WorkflowInstance spawned
  AND: OrchestratorState.active contains instance_id
  AND: recovered instance status.phase == Live
  AND: recovered instance total_events_applied == pre-crash value
  AND: recovered FSM current_state == "Authorized"
```

**Teardown:**
- Signal shutdown to heartbeat watcher
- Stop MasterOrchestrator
- Clean up temp sled dir

### E2E-2: `no_recovery_when_instance_active`

**Setup:** Same as E2E-1

**Test Steps:**
```
GIVEN: WorkflowInstance running and active
  AND: Heartbeat watcher running

WHEN: Manually cast HeartbeatExpired { instance_id } to orchestrator

THEN: No recovery attempted (check_recovery_preconditions skips)
  AND: active_count() == 1 (unchanged)
  AND: No "wf-recovered-" actor spawned
```

### E2E-3: `heartbeat_watcher_shutdown_clean`

**Test Steps:**
```
GIVEN: Heartbeat watcher running with shutdown_rx

WHEN: Send shutdown signal (shutdown_rx.send(true))

THEN: run_heartbeat_watcher returns Ok(())
  AND: No panic or error
```

---

## 5.5 Verification

| Gate | Command | Criteria |
|------|---------|----------|
| Compile | `cargo check -p vo-actor` | Zero errors, zero warnings |
| Unit tests | `cargo test -p vo-actor` | All pass |
| E2E tests | `cargo test -p vo-actor --test heartbeat_expiry_recovery -- --nocapture` | Pass (requires NATS) |
| Clippy | `cargo clippy -p vo-actor -- -D warnings` | Zero warnings |
| Full workspace | `cargo test --workspace` | No regressions |

---

## 6 Implementation Tasks

| # | Task | File | Est |
|---|------|------|-----|
| 1 | Create `crates/vo-actor/tests/heartbeat_expiry_recovery.rs` scaffolding | new file | 15m |
| 2 | Implement NATS test harness (connect, provision streams + KV, sled tempdir) | test file | 30m |
| 3 | Build `OrchestratorConfig` with real stores for test | test file | 20m |
| 4 | Spawn `MasterOrchestrator` + heartbeat watcher in test setup | test file | 20m |
| 5 | Implement E2E-1: start FSM workflow, publish events, kill, wait for expiry, verify recovery | test file | 60m |
| 6 | Implement E2E-2: verify no-recovery-when-active | test file | 15m |
| 7 | Implement E2E-3: verify clean shutdown | test file | 10m |
| 8 | Add unit tests T1–T5 | test file | 30m |
| 9 | Run verification gates (compile, test, clippy) | — | 10m |

**Total: ~4hr**

---

## 7 Failure Modes

| # | Failure | Impact | Mitigation |
|---|---------|--------|------------|
| F1 | NATS not running | All E2E tests fail to connect | Guard test with `#[ignore]` or skip if connection fails; document NATS requirement |
| F2 | Heartbeat TTL not exactly 10s | Timing-dependent test flake | Use generous timeout (15s) + poll loop instead of exact sleep |
| F3 | Supervisor deregistration race | Recovery skipped (instance still in active) | Sleep 200ms after kill; poll `active.is_empty()` before waiting for expiry |
| F4 | KV bucket not yet created | Watcher fails with "bucket not found" | Provision KV in test setup before spawning watcher |
| F5 | Multiple heartbeat watchers (port reuse) | Duplicate HeartbeatExpired events | Each test creates unique bucket name or purges existing entries |
| F6 | sled snapshot corruption | Recovery loads wrong state | Test works with or without snapshots; verify state via event replay |
| F7 | JetStream consumer leak | Test hangs on teardown | Ensure all consumers are dropped; use ephemeral consumers only |

---

## 7.5 Anti-Hallucination

| # | Claim | Verification |
|---|-------|-------------|
| AH1 | `vo-heartbeats` has `max_age: 10s` | Confirmed: `crates/vo-storage/src/kv.rs:101` |
| AH2 | Heartbeat interval is 5s | Confirmed: `crates/vo-actor/src/instance/actor.rs:54` |
| AH3 | Key format is `hb/{instance_id}` | Confirmed: `crates/vo-storage/src/kv.rs:149` (`heartbeat_key()`) |
| AH4 | `handle_heartbeat_expired` exists | Confirmed: `crates/vo-actor/src/master/handlers/heartbeat.rs:68` |
| AH5 | Recovery fetches `InstanceMetadata` | Confirmed: `crates/vo-actor/src/master/handlers/heartbeat.rs:79` (`fetch_metadata`) |
| AH6 | `OrchestratorState.active` is `HashMap<InstanceId, ActorRef<InstanceMsg>>` | Confirmed: `crates/vo-actor/src/master/state.rs:42` |
| AH7 | `WorkflowInstance::spawn_linked` exists | Confirmed: `crates/vo-actor/src/master/handlers/heartbeat.rs:59` |
| AH8 | `handle_heartbeat` writes via `state_store.put_heartbeat` | Confirmed: `crates/vo-actor/src/instance/handlers.rs:133` |
| AH9 | `post_stop` aborts `procedural_task` and `live_subscription_task` | Confirmed: `crates/vo-actor/src/instance/actor.rs:79-84` |
| AH10 | `SNAPSHOT_INTERVAL = 100` | Confirmed: `crates/vo-actor/src/instance/handlers.rs:193` |

---

## 7.6 Context Survival

If the agent loses context mid-implementation:

1. **Read this spec** at `.beads/vo-5fow/spec.md`
2. **Read the primary source files**:
   - `crates/vo-actor/src/heartbeat.rs` — watcher implementation
   - `crates/vo-actor/src/master/handlers/heartbeat.rs` — recovery handler
   - `crates/vo-actor/src/instance/actor.rs` — WorkflowInstance actor
   - `crates/vo-storage/src/kv.rs` — KV bucket provisioning
3. **Check existing test patterns** at `crates/vo-actor/tests/spawn_workflow_test.rs`
4. **Key types to reference**:
   - `vo_actor::OrchestratorMsg::HeartbeatExpired { instance_id }`
   - `vo_actor::heartbeat::run_heartbeat_watcher(heartbeats, orchestrator, shutdown_rx)`
   - `vo_storage::kv::provision_kv_buckets(js)`
   - `vo_storage::provision::provision_streams(js)`
   - `vo_storage::snapshots::open_snapshot_db(path)`
5. **NATS connection**: `async_nats::connect("nats://localhost:4222").await`

---

## 8 Completion

- [ ] `crates/vo-actor/tests/heartbeat_expiry_recovery.rs` created
- [ ] E2E-1 `crash_recovery_fsm_heartbeat_expiry` passes with NATS
- [ ] E2E-2 `no_recovery_when_instance_active` passes
- [ ] E2E-3 `heartbeat_watcher_shutdown_clean` passes
- [ ] Unit tests T1–T5 pass
- [ ] `cargo clippy -p vo-actor -- -D warnings` clean
- [ ] `cargo test --workspace` no regressions
- [ ] Test file < 300 lines (architectural drift check)

---

## 9 Context

**Project:** vo-engine — durable execution runtime (Rust, Ractor, NATS JetStream)
**Bead ID:** vo-5fow
**Title:** e2e: Test crash recovery via heartbeat expiry
**Priority:** 1 (High — validates core crash recovery path)
**Type:** task
**Effort:** 4hr
**Dependencies:** None (requires NATS running)
**Crate:** `vo-actor` (test file in `crates/vo-actor/tests/`)

---

## 10 AI Hints

1. **Use `ractor::Actor::spawn` to create the orchestrator** — see `crates/vo-actor/src/master/mod.rs` for the `MasterOrchestrator` struct.
2. **The heartbeat watcher is a plain async function** (not an actor) — spawn it via `tokio::spawn` and communicate shutdown via `tokio::sync::watch`.
3. **For FSM test workflow**: Create a simple 2-state FSM definition (Created → Authorized) and register it in `WorkflowRegistry` before starting the orchestrator.
4. **To publish events directly**: Use the `EventStore` trait impl to publish `WorkflowEvent::InstanceStarted` and `WorkflowEvent::TransitionApplied` events to the `vo-events` stream.
5. **To kill the instance**: Get `ActorRef<InstanceMsg>` from `OrchestratorState.get(&instance_id)`, call `.stop(Some("crash".into()))`. Then wait briefly for the supervision `ActorTerminated` event to deregister it.
6. **To verify recovery**: After 15s, use `OrchestratorMsg::GetStatus { instance_id, reply }` to query the recovered instance's state.
7. **sled tempdir**: Use `tempfile::tempdir()` and pass the path to `vo_storage::snapshots::open_snapshot_db`.
8. **Mark E2E tests with `#[tokio::test]`** and add `#[ignore]` annotation if NATS may not be available in CI.
9. **The `in_flight_guard` in `check_recovery_preconditions` uses a process-level `static OnceLock<Mutex<HashSet>>`** — it persists across tests within the same process. Each test must use a unique `instance_id` to avoid interference.
