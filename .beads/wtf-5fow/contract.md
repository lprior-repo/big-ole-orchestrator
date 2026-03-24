# Contract Specification: E2E Crash Recovery via Heartbeat Expiry

## Context

- **Feature:** End-to-end test verifying the crash recovery path triggered by heartbeat TTL expiry in NATS KV
- **Domain terms:**
  - `WorkflowInstance` — Ractor actor executing a single workflow instance (FSM, DAG, or Procedural)
  - `MasterOrchestrator` — Root supervisor actor that spawns and tracks WorkflowInstance actors
  - `HeartbeatExpired` — Message sent to orchestrator when a KV entry is TTL-deleted
  - `in_flight_guard` — Process-level `OnceLock<Mutex<HashSet<String>>>` preventing duplicate recovery
  - `OrchestratorState.active` — `HashMap<InstanceId, ActorRef<InstanceMsg>>` of currently running instances
  - `wtf-heartbeats` — NATS KV bucket with `max_age = 10s`, `storage = Memory`, `history = 1`
  - `ParadigmState` — Discriminated enum holding FSM/DAG/Procedural state
- **Assumptions:**
  - NATS is running (`wtf-nats-test` container on port 4222)
  - Heartbeat interval is 5s (`actor.rs:56`), TTL is 10s (`kv.rs:101`)
  - FSM paradigm used for E2E test (simplest state reconstruction)
  - sled snapshot DB provisioned via `tempfile::tempdir()`
  - Recovery path: watcher -> HeartbeatExpired -> check_preconditions -> fetch_metadata -> spawn_linked
- **Open questions:** None — all answered in spec.md clarifications

---

## Preconditions

### Global (all tests)
- [P1] NATS server reachable at `nats://localhost:4222`
- [P2] JetStream context obtained from NATS connection
- [P3] `wtf-events` stream provisioned via `wtf_storage::provision::provision_streams(js)`
- [P4] `wtf-heartbeats` KV bucket provisioned via `wtf_storage::kv::provision_kv_buckets(js)` with `max_age: 10s`
- [P5] sled snapshot database opened at a temp directory path
- [P6] `OrchestratorConfig` built with real `event_store`, `state_store`, `snapshot_db`, `task_queue`
- [P7] `MasterOrchestrator` spawned via `Actor::spawn` with linked supervision
- [P8] FSM workflow definition ("checkout-fsm") registered in `WorkflowRegistry`

### Per-test
- [P9] Heartbeat watcher task started via `tokio::spawn(run_heartbeat_watcher(heartbeats, orchestrator, shutdown_rx))`
- [P10] Unique `instance_id` per test (prevents in_flight_guard collision across tests in same process)

---

## Postconditions

### E2E-1: Crash Recovery Completes
- [Q1] After heartbeat KV entry expires (<=15s after instance kill), the watcher sends `HeartbeatExpired { instance_id }` to the orchestrator exactly once
- [Q2] `check_recovery_preconditions` returns `Some(in_flight_key)` because `state.active` no longer contains the killed instance
- [Q3] `fetch_metadata` returns `Some(InstanceMetadata)` with `instance_id`, `namespace`, `workflow_type`, `paradigm` matching original
- [Q4] `WorkflowInstance::spawn_linked` is called with actor name `"wf-recovered-{instance_id}"`
- [Q5] Recovered instance enters `InstancePhase::Live`
- [Q6] Recovered instance `total_events_applied` equals the pre-crash value (deterministic replay)
- [Q7] Recovered instance FSM `paradigm_state.current_state` equals the pre-crash state
- [Q8] `OrchestratorState.active` contains exactly one entry for `instance_id` after recovery
- [Q9] `in_flight_guard` is cleaned up (instance_id key removed from the `HashSet`)

### E2E-2: No Recovery When Instance Active
- [Q10] After casting `HeartbeatExpired` while instance is still active, no new actor is spawned
- [Q11] `OrchestratorState.active_count()` remains 1 (unchanged)
- [Q12] No `"wf-recovered-"` actor appears in active map

### E2E-3: Watcher Shutdown Clean
- [Q13] `run_heartbeat_watcher` returns `Ok(())` when `shutdown_rx` fires
- [Q14] No panic or error logged during shutdown

---

## Invariants

- [I1] KV key format is always `"hb/{instance_id}"` — never any other prefix in `wtf-heartbeats` bucket
- [I2] Heartbeat watcher NEVER sends `HeartbeatExpired` for `Operation::Put` entries (only `Delete | Purge`)
- [I3] Heartbeat watcher NEVER sends `HeartbeatExpired` for keys that do not start with `"hb/"`
- [I4] `in_flight_guard` set is process-global (`static OnceLock`) — persists across all tests in same binary
- [I5] `post_stop` always aborts `procedural_task` and `live_subscription_task`, preventing further heartbeat writes
- [I6] `handle_child_termination` deregisters from `OrchestratorState.active` before the watcher can trigger recovery (supervision event ordering)
- [I7] Deterministic replay: given the same events 1..=M, `load_initial_state` + `replay_events` always produces the same `paradigm_state` and `total_events_applied == M`
- [I8] `publish_instance_started` is skipped when `event_log` is non-empty (crash recovery), preventing duplicate `InstanceStarted` events
- [I9] `total_events_applied` is monotonically increasing — never decreases

---

## Error Taxonomy

| Variant | When | Source |
|---------|------|--------|
| `WtfError::NatsPublish` | KV bucket creation fails, heartbeat write/delete fails, event publish fails | `wtf-storage/src/kv.rs`, `wtf-storage/src/journal.rs` |
| `ActorProcessingErr` | `pre_start` fails during `load_initial_state`, `replay_events`, or `transition_to_live` | `ractor` framework |
| `Err(String)` | `run_heartbeat_watcher` initial `watch_all()` fails | `heartbeat.rs:63` |
| Recovery skipped (no error) | `check_recovery_preconditions` returns `None` — instance still active or in-flight guard blocks | `heartbeat.rs:27-38` |
| Recovery skipped (no error) | `fetch_metadata` returns `None` — no `InstanceMetadata` in state store | `heartbeat.rs:48-52` |
| Recovery spawn failure (logged, no error) | `WorkflowInstance::spawn_linked` returns `Err` — `in_flight_guard` still cleaned up | `heartbeat.rs:58-65` |

---

## Contract Signatures

### Public API surface used in tests

```rust
// heartbeat.rs — watcher entry point
pub async fn run_heartbeat_watcher(
    heartbeats: Store,
    orchestrator: ActorRef<OrchestratorMsg>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String>

// heartbeat.rs — key parser (pure function)
pub fn instance_id_from_heartbeat_key(key: &str) -> Option<InstanceId>

// master/handlers/heartbeat.rs — recovery handler
pub async fn handle_heartbeat_expired(
    myself: ActorRef<OrchestratorMsg>,
    state: &mut OrchestratorState,
    instance_id: InstanceId,
)

// master/mod.rs — orchestrator actor
impl Actor for MasterOrchestrator {
    type Msg = OrchestratorMsg;
    type State = OrchestratorState;
    type Arguments = OrchestratorConfig;
}

// instance/actor.rs — workflow instance actor
impl Actor for WorkflowInstance {
    type Msg = InstanceMsg;
    type State = InstanceState;
    type Arguments = InstanceArguments;
}

// messages/orchestrator.rs — messages
OrchestratorMsg::HeartbeatExpired { instance_id: InstanceId }
OrchestratorMsg::StartWorkflow { namespace, instance_id, workflow_type, paradigm, input, reply }
OrchestratorMsg::GetStatus { instance_id, reply }
OrchestratorMsg::ListActive { reply }

// messages/instance.rs
InstanceMsg::Heartbeat
InstanceMsg::GetStatus(RpcReplyPort<InstanceStatusSnapshot>)

// storage/kv.rs — KV operations
pub async fn provision_kv_buckets(js: &Context) -> Result<KvStores, WtfError>
pub async fn write_heartbeat(heartbeats: &Store, instance_id: &InstanceId, engine_node_id: &str) -> Result<(), WtfError>
pub fn heartbeat_key(instance_id: &InstanceId) -> String

// storage/provision.rs
pub async fn provision_streams(js: &Context) -> Result<(), WtfError>

// storage/snapshots.rs
pub fn open_snapshot_db(path: &std::path::Path) -> Result<sled::Db, WtfError>

// master/state.rs — orchestrator state management
impl OrchestratorState {
    pub fn active_count(&self) -> usize
    pub fn register(&mut self, id: InstanceId, actor_ref: ActorRef<InstanceMsg>)
    pub fn deregister(&mut self, id: &InstanceId)
    pub fn build_instance_args(&self, seed: InstanceSeed) -> InstanceArguments
}
```

---

## Non-goals

- [NG1] Do NOT test DAG or Procedural paradigm recovery (only FSM)
- [NG2] Do NOT test snapshot-specific recovery (test must work with full replay only)
- [NG3] Do NOT test cross-node recovery (single-node only)
- [NG4] Do NOT test NATS cluster failover or KV replication
- [NG5] Do NOT test `OrchestratorMsg::Terminate` path (this is graceful, not crash)
- [NG6] Do NOT verify exact timing of heartbeat expiry (use generous timeout, not exact sleep)
- [NG7] Do NOT test in-flight guard behavior across multiple processes (process-local only)
