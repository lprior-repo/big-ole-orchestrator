# Martin Fowler Test Plan: E2E Crash Recovery via Heartbeat Expiry

**Bead:** vo-5fow
**Test file:** `crates/vo-actor/tests/heartbeat_expiry_recovery.rs`
**Paradigm:** FSM
**NATS:** Required (`vo-nats-test`, port 4222)

---

## Unit Tests (pure logic, no NATS)

### U1: `heartbeat_key_parsed_correctly_for_recovery`
**Type:** Happy path
**Source contract:** [I1], `instance_id_from_heartbeat_key` signature

```
Given: a heartbeat KV key "hb/crash-test-inst-001"
When:  instance_id_from_heartbeat_key("hb/crash-test-inst-001") is called
Then:  returns Some(InstanceId) where id.as_str() == "crash-test-inst-001"
```

### U2: `heartbeat_key_missing_prefix_returns_none`
**Type:** Error path
**Source contract:** [I3]

```
Given: a key "instance/01ARZ" without the "hb/" prefix
When:  instance_id_from_heartbeat_key("instance/01ARZ") is called
Then:  returns None
```

### U3: `heartbeat_key_empty_string_returns_none`
**Type:** Edge case
**Source contract:** [I3]

```
Given: an empty string key ""
When:  instance_id_from_heartbeat_key("") is called
Then:  returns None
```

### U4: `recovery_args_preserve_metadata_fields`
**Type:** Happy path
**Source contract:** [Q3], C3 from spec

```
Given: an InstanceMetadata { namespace: "test", instance_id: "inst-1",
        workflow_type: "checkout", paradigm: Fsm, engine_node_id: "node-1" }
When:  build_recovery_args(state, &metadata) is called
Then:  resulting InstanceArguments has matching namespace, instance_id,
        workflow_type, paradigm
  And:  engine_node_id comes from state.config.engine_node_id
```

### U5: `recovery_skipped_when_instance_still_active`
**Type:** Contract verification (precondition)
**Source contract:** [Q10], [Q11], [Q12], C2 from spec

```
Given: OrchestratorState with instance_id "inst-x" in state.active
When:  check_recovery_preconditions(state, &instance_id) is called
Then:  returns None (recovery skipped)
  And:  state.active is not modified
```

### U6: `in_flight_guard_blocks_duplicate_recovery`
**Type:** Contract verification (precondition)
**Source contract:** [I4], C2 from spec

```
Given: in_flight_guard already contains "inst-x" from a prior call
When:  check_recovery_preconditions(state, &instance_id) is called
        (with instance_id NOT in state.active)
Then:  returns None (duplicate blocked)
  And:  in_flight_guard set is not modified
```

### U7: `heartbeat_key_format_is_hb_prefix`
**Type:** Invariant verification
**Source contract:** [I1]

```
Given: any InstanceId "abc-123"
When:  heartbeat_key(&id) is called
Then:  result starts with "hb/" and ends with "abc-123"
```

---

## Integration Tests (live NATS required)

### I1: `crash_recovery_fsm_heartbeat_expiry` — PRIMARY E2E
**Type:** Happy path (end-to-end)
**Source contract:** [Q1]-[Q9], [I5]-[I9], E1-E5 from spec
**Annotation:** `#[tokio::test] #[ignore]` (requires NATS)

```
GIVEN: MasterOrchestrator running with real event_store, state_store, snapshot_db
  AND:  heartbeat watcher running in background tokio task
  AND:  FSM workflow "checkout-fsm" registered (Created -> Authorized)
  AND:  unique instance_id = "e2e-crash-fsm-001"

WHEN:  Start workflow via OrchestratorMsg::StartWorkflow { paradigm: Fsm, ... }
  AND:  Instance enters InstancePhase::Live
  AND:  Publish TransitionApplied { from: "Created", to: "Authorized" } event
  AND:  Record pre-crash total_events_applied (expect >= 2: InstanceStarted + TransitionApplied)
  AND:  Record pre-crash FSM current_state (expect "Authorized")

WHEN:  Kill WorkflowInstance via ActorRef::stop(Some("simulated crash".into()))
  AND:  Wait 200ms for supervisor ActorTerminated to deregister from OrchestratorState.active
  AND:  Verify active_count() == 0

WHEN:  Wait up to 15s (polling every 500ms) for heartbeat KV entry to expire (max_age=10s)
  AND:  Heartbeat watcher detects Delete operation
  AND:  Orchestrator receives HeartbeatExpired { instance_id }

THEN:  check_recovery_preconditions returns Some(in_flight_key)
        (active is empty, guard is clean)
  AND:  fetch_metadata returns Some(InstanceMetadata)
  AND:  WorkflowInstance::spawn_linked called with name "wf-recovered-e2e-crash-fsm-001"
  AND:  Recovered instance enters InstancePhase::Live (poll GetStatus, timeout 10s)
  AND:  Recovered instance total_events_applied == pre-crash value
  AND:  Recovered FSM current_state == "Authorized"
  AND:  OrchestratorState.active_count() == 1
  AND:  in_flight_guard no longer contains "e2e-crash-fsm-001"
```

### I2: `no_recovery_when_instance_active`
**Type:** Error path (precondition violation)
**Source contract:** [Q10]-[Q12], E6 from spec
**Annotation:** `#[tokio::test] #[ignore]`

```
GIVEN: MasterOrchestrator running with heartbeat watcher
  AND:  unique instance_id = "e2e-no-recover-001"
  AND:  WorkflowInstance started and in InstancePhase::Live

WHEN:  Manually cast OrchestratorMsg::HeartbeatExpired { instance_id }
        to orchestrator (while instance is still running and heartbeat is fresh)

THEN:  No new actor is spawned
  AND:  OrchestratorState.active_count() == 1 (unchanged)
  AND:  No "wf-recovered-" actor appears in active map
  AND:  in_flight_guard does NOT contain "e2e-no-recover-001"
        (guard was never acquired because active check failed first)
```

### I3: `heartbeat_watcher_shutdown_clean`
**Type:** Happy path (lifecycle)
**Source contract:** [Q13], [Q14]
**Annotation:** `#[tokio::test] #[ignore]`

```
GIVEN: Heartbeat watcher running via tokio::spawn
  AND:  shutdown_rx watch channel connected

WHEN:  Send shutdown signal via shutdown_tx.send(true)

THEN:  run_heartbeat_watcher returns Ok(())
  AND:  No panic or error logged
  AND:  tokio task handle completes (join does not timeout)
```

### I4: `duplicate_heartbeat_expired_triggers_single_recovery`
**Type:** Edge case (deduplication)
**Source contract:** [I4], [Q9]
**Annotation:** `#[tokio::test] #[ignore]`

```
GIVEN: MasterOrchestrator running
  AND:  unique instance_id = "e2e-dedup-001"
  AND:  WorkflowInstance was previously started, killed, and deregistered
  AND:  InstanceMetadata exists in state store
  AND:  First HeartbeatExpired already triggered recovery (recovered instance in active)

WHEN:  Second HeartbeatExpired { instance_id: "e2e-dedup-001" } arrives
        (simulates watcher race or duplicate KV notification)

THEN:  check_recovery_preconditions returns None (instance now in active)
  AND:  active_count() == 1 (no duplicate spawned)
```

---

## Contract Verification Tests

### CV1: `watcher_ignores_put_operations`
**Source contract:** [I2]

```
Given: heartbeat watcher running
  AND:  a fresh heartbeat entry written via write_heartbeat()
When:  watcher processes the Put operation
Then:  NO HeartbeatExpired message is sent to orchestrator
```

### CV2: `watcher_ignores_non_hb_keys`
**Source contract:** [I3]

```
Given: heartbeat watcher running
  AND:  an entry with key "not-a-heartbeat/abc" is written and then deleted
When:  watcher processes the Delete operation
Then:  NO HeartbeatExpired message is sent to orchestrator
```

### CV3: `total_events_applied_monotonically_increasing`
**Source contract:** [I9]

```
Given: FSM workflow with 3 events published sequentially
When:  each event is applied via inject_event
Then:  total_events_applied is 1, then 2, then 3
  And:  it never decreases
```

### CV4: `publish_instance_started_skipped_on_recovery`
**Source contract:** [I8]

```
Given: crash recovery scenario where event_log is non-empty (2 events replayed)
When:  publish_instance_started is called during pre_start
Then:  NO InstanceStarted event is published to the event store
  And:  function returns Ok(())
```

### CV5: `in_flight_guard_cleaned_up_on_spawn_failure`
**Source contract:** [Q9]

```
Given: check_recovery_preconditions inserts "inst-x" into in_flight_guard
  AND:  WorkflowInstance::spawn_linked returns Err (simulated bad args)
When:  attempt_recovery completes
Then:  in_flight_guard no longer contains "inst-x"
  And:  no key leak
```

### CV6: `deregister_happens_before_recovery_trigger`
**Source contract:** [I6]

```
Given: WorkflowInstance is linked to MasterOrchestrator (supervision)
When:  WorkflowInstance.stop() is called
Then:  MasterOrchestrator.handle_supervisor_evt(ActorTerminated) fires
  And:  state.deregister(&instance_id) is called
  And:  state.active no longer contains instance_id
  And:  this all completes within 200ms (test waits before checking)
```

---

## Inversion Detection Tests

### ID1: `watcher_receives_delete_within_timeout`
**Source inversion:** I1 from spec — watcher misses Delete event due to race

```
Given: heartbeat KV entry exists for instance
  AND:  no further writes to that entry (actor is dead)
When:  15s elapses (10s TTL + 5s margin)
Then:  watcher MUST have received at least one Delete or Purge operation
        for the key "hb/{instance_id}"
  And:  assertion fails if no HeartbeatExpired was cast within 15s
```

### ID2: `recovery_produces_single_active_instance`
**Source inversion:** I2 from spec — duplicate spawn due to guard failure

```
Given: crash recovery completed for instance_id
When:  OrchestratorState.active is queried
Then:  exactly one ActorRef exists for instance_id
  And:  no ActorRef with name starting "wf-recovered-" duplicates
```

### ID3: `recovered_state_matches_pre_crash`
**Source inversion:** I3 from spec — replay bug or snapshot corruption

```
Given: pre-crash FSM state is "Authorized" with total_events_applied == 2
When:  recovered instance reaches Live phase
Then:  FSM current_state == "Authorized"
  And:  total_events_applied == 2
  And:  no event was skipped or duplicated during replay
```

### ID4: `active_cleared_after_supervised_stop`
**Source inversion:** I4 from spec — ActorTerminated supervision event not processed

```
Given: WorkflowInstance linked to MasterOrchestrator
When:  instance.stop() is called
  And:  200ms elapses
Then:  OrchestratorState.active_count() == 0
  And:  if not, test fails with "supervision deregistration race"
```

---

## Test Infrastructure

### Shared Test Harness

All integration tests (I1-I4) share this setup:

```rust
async fn setup_test_infrastructure() -> TestHarness {
    // 1. Connect NATS: async_nats::connect("nats://localhost:4222").await
    // 2. Create JetStream context
    // 3. provision_streams(js) — creates vo-events stream
    // 4. provision_kv_buckets(js) — creates vo-heartbeats (max_age=10s)
    // 5. Open sled snapshot db: tempfile::tempdir() + open_snapshot_db()
    // 6. Build OrchestratorConfig with real stores
    // 7. Register FSM definition "checkout-fsm" (Created -> Authorized)
    // 8. Spawn MasterOrchestrator
    // 9. Create watch::channel for heartbeat watcher shutdown
    // 10. Spawn run_heartbeat_watcher as tokio task
    // Returns TestHarness { orchestrator, heartbeats, shutdown_tx, snapshot_dir, ... }
}

struct TestHarness {
    orchestrator: ActorRef<OrchestratorMsg>,
    heartbeats: Store,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    snapshot_dir: tempfile::TempDir,
    instance_id: InstanceId,
}
```

### Teardown

```rust
impl Drop for TestHarness {
    fn drop(&mut self) {
        // 1. Send shutdown signal to heartbeat watcher
        // 2. Stop MasterOrchestrator
        // 3. snapshot_dir auto-cleaned by tempfile::TempDir
    }
}
```

---

## Test Execution Order

| Order | Test | Type | NATS? | Timeout Budget |
|-------|------|------|-------|----------------|
| 1 | U1-U3 | Unit (pure) | No | < 1s each |
| 2 | U4-U6 | Unit (state logic) | No | < 1s each |
| 3 | U7 | Unit (key format) | No | < 1s |
| 4 | CV1-CV6 | Contract verification | No (mocked) | < 1s each |
| 5 | I3 | Integration (shutdown) | Yes | 5s |
| 6 | I2 | Integration (no-recovery) | Yes | 15s |
| 7 | I4 | Integration (dedup) | Yes | 15s |
| 8 | I1 | Integration (primary E2E) | Yes | 30s (10s TTL + 15s poll + margin) |
| 9 | ID1-ID4 | Inversion detection | Yes | 30s each |

---

## Verification Commands

```bash
# Compile check
cargo check -p vo-actor

# Unit tests (no NATS)
cargo test -p vo-actor -- heartbeat_expiry_recovery

# E2E tests (requires NATS)
cargo test -p vo-actor --test heartbeat_expiry_recovery -- --nocapture

# E2E tests with ignored tests included
cargo test -p vo-actor --test heartbeat_expiry_recovery -- --nocapture --ignored

# Clippy
cargo clippy -p vo-actor -- -D warnings

# Full workspace regression
cargo test --workspace
```
