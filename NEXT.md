# wtf-engine: What's Next

> Last updated: 2026-03-23
> Verified by: Red Queen full-arsenal audit (28 challengers, cargo-audit, cargo-geiger, rust-code-analysis-cli)
> Test baseline: 395 tests, 0 failures, 0 unsafe

---

## Reference: Current State

### What Actually Works Against Real NATS

| Layer | Status | Evidence |
|-------|--------|----------|
| NATS connection + retry (3x, exponential backoff) | ✅ Real | `wtf-storage/src/nats.rs:74-116` |
| JetStream: 4 streams (events, work, signals, archive) | ✅ Real | `wtf-storage/src/provision.rs:30-36` |
| KV: 4 buckets (instances, timers, definitions, heartbeats) | ✅ Real | `wtf-storage/src/kv.rs:38-45` |
| Event journal: append + replay consumer | ✅ Real | 18 integration tests in `wtf-storage/tests/` |
| Sled snapshots: write/read/delete with CRC | ✅ Real | `wtf-storage/src/snapshots.rs` |
| Work queue: enqueue/pull/ack/nak/retry | ✅ Real | 16 integration tests in `wtf-worker/tests/` |
| Timer loop: poll KV + fire expired | ✅ Real | `wtf-worker/src/timer/loop.rs` |
| Instance spawn + linked supervision | ✅ Real | `wtf-actor/src/master/handlers/start.rs:53-77` |
| Heartbeat writes: every 5s, TTL=10s | ✅ Real | `wtf-actor/src/instance/handlers.rs:131-141` |
| HTTP API: 12 routes (CRUD, journal, signal, replay-to, SSE) | ✅ Real | `wtf-api/src/app.rs:48-73` |
| FSM engine: state transitions, guard eval, journal replay | ✅ Real | `wtf-actor/src/fsm/` (isolated) |
| DAG engine: parallel dispatch, completion tracking | ✅ Real | `wtf-actor/src/dag/` (isolated) |
| Procedural engine: sequential steps, timers, signals, checkpoints | ✅ Real | `wtf-actor/src/procedural/` (isolated) |
| Activity completion round-trip through JetStream | ✅ Real | `wtf-worker/src/worker.rs:230-302` |
| Linter: 6 procedural workflow rules | ✅ Real | `wtf-linter/` |

### What Doesn't Work (Cannot Complete a Workflow)

| # | Gap | Where | File:Line |
|---|-----|-------|-----------|
| 1 | **No workflow definitions loaded** — `ingest_definition` only lints, never stores in KV. Registry is in-memory HashMap, nothing loads from KV on startup. `graph_raw` never parsed into FsmDefinition/DagActorState. FSM starts with zero transitions, DAG starts empty → both immediately terminal. | `wtf-api/src/handlers/definitions.rs` `wtf-actor/src/master/registry.rs` `wtf-actor/src/master/state.rs:106` | See Action Item 1 |
| 2 | **Heartbeat watcher never started** — `run_heartbeat_watcher` exists and is fully tested but is never called from `serve.rs`. Crash recovery never triggers. | `wtf-actor/src/heartbeat.rs:55` `wtf-cli/src/commands/serve.rs` | See Action Item 2 |
| 3 | **Signal handler is a stub** — logs "(stub)" and returns Ok without actually delivering the signal to the procedural wait. | `wtf-actor/src/instance/handlers.rs:116-129` | See Action Item 3 |
| 4 | **Snapshot trigger is a stub** — logs "stub -- see wtf-flbh" and resets counter without writing to sled. | `wtf-actor/src/instance/handlers.rs:215-222` | See Action Item 4 |
| 5 | **No workers started by `wtf serve`** — Worker SDK is real (16 passing integration tests) but `serve.rs` doesn't start any. Activity tasks go to JetStream but nobody picks them up. | `wtf-cli/src/commands/serve.rs` | See Action Item 5 |
| 6 | **`InstanceStarted` event never published** — First event in the log is missing. Consumers/replay that expect it will break. | `wtf-actor/src/instance/init.rs:13-61` | See Action Item 6 |
| 7 | **No E2E integration test** — No test exercises HTTP start → instance spawn → activity dispatch → worker execution → completion against real NATS. | None | See Action Item 7 |

---

## Action Items (Ordered by Dependency)

### 🔴 CRITICAL PATH (must be done in order)

#### Action Item 1: Wire definition storage + registry loading
**Depends on:** Nothing
**Unblocks:** Items 2, 3, 5, 7 (E2E test)

What's broken:
- `POST /api/v1/definitions/:type` (`definitions.rs:5-37`) only lints, never stores the definition in the `wtf-definitions` KV bucket
- `WorkflowRegistry` (`registry.rs:8-13`) is an in-memory HashMap with `procedural` and `definitions` maps
- Nothing loads definitions from KV into registry on startup
- `build_instance_args` (`state.rs:106`) calls `self.registry.get_definition(&seed.workflow_type)` which always returns None
- FSM starts with zero transitions (`state.rs:61`), DAG starts with empty node set (`state.rs:62-64`) → both immediately terminal

What to do:
1. After `ingest_definition` passes lint, store the source in KV bucket `wtf-definitions` with key = workflow_type
2. On `serve` startup, after KV buckets are provisioned, scan `wtf-definitions` bucket and call `registry.register_definition(name, WorkflowDefinition { paradigm, graph_raw, description })` for each
3. The FSM parser: parse `graph_raw` into `FsmDefinition` by extracting transitions and terminal states (see `fsm/definition.rs:6-38` for the target struct)
4. The DAG parser: parse `graph_raw` into node set for `DagActorState` initialization (see `dag/mod.rs` for the state struct)

**Files to touch:** `definitions.rs`, `registry.rs`, `serve.rs`, possibly new `parsers.rs`

#### Action Item 2: Start heartbeat watcher in serve.rs
**Depends on:** Nothing
**Unblocks:** Item 7 (E2E test needs crash recovery)

What to do:
1. `run_heartbeat_watcher` takes `(Store, ActorRef<OrchestratorMsg>, Receiver<bool>)` — all three are available in `serve.rs`
2. Add one `tokio::spawn` call with a cloned `shutdown_rx` channel
3. That's it. One line + import.

**Files to touch:** `serve.rs`

#### Action Item 3: Implement signal delivery in procedural workflows
**Depends on:** Item 1 (procedural workflow must be registered to test)
**Unblocks:** Item 7 (E2E test with signal-receiving workflows)

What's broken:
- `handle_signal` at `instance/handlers.rs:116-129` logs "(stub)" and returns `Ok(())`
- Procedural workflows have `ctx.wait_for_signal("signal_name")` in the `WorkflowContext` API
- The signal must be routed to any pending `wait_for_signal` call in the procedural task

What to do:
1. Store the signal in `InstanceState` (e.g., a `HashMap<String, Bytes>` or a channel)
2. When `wait_for_signal` is called, check if the signal is already pending
3. If not, register as a waiter and return Pending (checkpoint)
4. When `handle_signal` is called, wake the waiter

**Files to touch:** `instance/handlers.rs`, `instance/state.rs`, `procedural/context.rs`

#### Action Item 4: Implement snapshot trigger
**Depends on:** Nothing
**Unblocks:** Item 7 (E2E test with long workflows)

What's broken:
- `handle_snapshot_trigger` at `instance/handlers.rs:215-222` logs "stub" and resets counter

What to do:
1. Call `self.snapshot_db` (which is `Option<Arc<sled::Db>>` already on InstanceArguments)
2. Serialize current paradigm state
3. Write to sled under the snapshot key
4. Reset `events_since_snapshot`

**Files to touch:** `instance/handlers.rs`

### 🟠 HIGH PRIORITY

#### Action Item 5: Start a built-in worker in serve.rs
**Depends on:** Items 1, 3, 4 (worker needs to handle real activities)
**Unblocks:** Item 7 (E2E test)

What to do:
1. The Worker SDK (`wtf-worker`) is designed for users to write custom workers
2. For E2E, `wtf serve` should start a default worker that handles basic activities
3. Minimal viable: a worker that handles "echo" and "sleep" activities to prove the dispatch chain works
4. Alternatively: document clearly that users must run a separate worker process

**Decision needed:** Should the engine include built-in activities or require users to write worker binaries?

#### Action Item 6: Publish InstanceStarted event
**Depends on:** Nothing
**Unblocks:** Item 7 (E2E test)

What's broken:
- `instance/init.rs:13-61` never publishes `WorkflowEvent::InstanceStarted`
- The event type exists in `wtf-common/src/events/mod.rs:18`

What to do:
1. After `spawn_live_subscription` succeeds (init.rs:94), publish `WorkflowEvent::InstanceStarted { instance_id, namespace, workflow_type, paradigm, input }`
2. One line: `store.publish(&ns, &id, event).await`

**Files to touch:** `instance/init.rs`

#### Action Item 7: E2E integration test against real NATS
**Depends on:** Items 1-6

What to do:
1. Start NATS in Docker (already running: `wtf-nats-test` on port 4222)
2. Start engine with `wtf serve`
3. Upload a simple procedural workflow definition
4. `POST /api/v1/workflows` to start it
5. Verify the workflow completes and events appear in journal
6. `GET /api/v1/workflows/:id/journal` to verify event log
7. Test terminate, signal, and crash recovery paths

**Files to touch:** New `tests/e2e_workflow_test.rs` or similar

---

## Backburner

### Definitions format
- Currently `graph_raw` is a plain String — no defined schema
- Need to decide: JSON? YAML? Custom DSL?
- Need parser for FSM transitions and DAG node declarations
- This is part of Action Item 1 but worth tracking separately

### Worker SDK documentation
- Worker SDK exists but has zero docs
- Users need to know how to write activity handlers
- `wtf-worker/tests/worker_integration_tests.rs` is the only reference

### CLI client commands
- 8 open beads for `run_start`, `run_status`, `run_signal`, `run_serve`
- Currently `wtf` only has `serve`, `lint`, and `admin`
- The HTTP API exists but there's no CLI to call it

### wtf-cli has 0 tests
- Known since project start, still zero
- `wtf-worker` also has 0 tests (SDK only, worker tests are in integration tests)

### Frontend
- Dioxus WASM shell exists with journal viewer, DAG simulator, procedural step-through
- No live data flow — reads from API but API isn't serving real data yet
- Backburner until the engine works E2E

### Retry policy
- `RetryPolicy` type exists in `wtf-common` but isn't used anywhere in the execution path
- Activity retries work (worker.rs:252-278) but workflow-level retry is not wired

---

## Execution Flow (What Happens When It Works)

```
User                Engine                     NATS                    Worker
────                ──────                     ────                    ──────

POST /definitions  ─→  lint()        ─→                               (validate)
                    ─→  store in KV  ─→  wtf-definitions
                    ─→  return 200

wtf serve starts    ─→  connect NATS
                    ─→  provision streams/KV
                    ─→  load definitions from KV into registry
                    ─→  start heartbeat watcher
                    ─→  start timer loop
                    ─→  start API server
                    ─→  start default worker

POST /workflows    ─→  validate request
                    ─→  MasterOrchestrator.StartWorkflow
                    ─→  spawn WorkflowInstance (linked)
                    ─→  persist metadata to KV
                    ─→  publish InstanceStarted  ─→  wtf.log
                    ─→  start heartbeat timer
                    ─→  return 201 + instance_id

Instance runs       ─→  step execution
                    ─→  dispatch activity        ─→  wtf.work ──────────────→  pull task
                    ─→  checkpoint/replay                                   execute
                    ─→  publish events           ─→  wtf.log
                    ─→  heartbeat tick          ─→  wtf.heartbeats
                    ─→  (on timer)              ─→  wtf.timers
                    ─→  snapshot trigger        ─→  sled

Worker completes    ─→                      ┌────────────── wtf.log (activity_completed)
                    ─→  inject event via live subscription
                    ─→  resume workflow

POST /signals      ─→  MasterOrchestrator.Signal
                    ─→  InstanceMsg.Cancel
                    ─→  wake pending wait_for_signal
                    ─→  resume workflow

DELETE /workflows  ─→  MasterOrchestrator.Terminate
                    ─→  InstanceMsg.Cancel
                    ─→  publish InstanceCancelled  ─→  wtf.log
                    ─→  stop actor
                    ─→  supervisor deregisters
                    ─→  return 204

Node crashes        ─→  heartbeat TTL expires
(heartbeat watcher) ─→  OrchestratorMsg.HeartbeatExpired
                    ─→  load metadata from KV
                    ─→  replay events from journal  ─→  wtf.log
                    ─→  re-spawn instance
                    ─→  resume from checkpoint
```

---

## Key File Index

| File | Purpose |
|------|---------|
| `wtf-cli/src/commands/serve.rs` | Server startup: NATS, streams, KV, orchestrator, API, timers |
| `wtf-api/src/handlers/workflow.rs` | HTTP handlers: start, get, terminate, list |
| `wtf-api/src/handlers/definitions.rs` | Definition lint-only endpoint (needs storage) |
| `wtf-actor/src/master/mod.rs` | MasterOrchestrator: spawn, supervise, route messages |
| `wtf-actor/src/master/state.rs` | OrchestratorState + OrchestratorConfig + InstanceSeed + build_instance_args |
| `wtf-actor/src/master/registry.rs` | WorkflowRegistry: in-memory HashMap (needs KV loading) |
| `wtf-actor/src/master/handlers/start.rs` | Spawn + register + persist metadata |
| `wtf-actor/src/master/handlers/heartbeat.rs` | Crash recovery: re-spawn on heartbeat expiry |
| `wtf-actor/src/instance/actor.rs` | WorkflowInstance: pre_start, handle, post_stop |
| `wtf-actor/src/instance/init.rs` | Load snapshot, replay events, start live subscription |
| `wtf-actor/src/instance/handlers.rs` | Message handlers: cancel (real), signal (stub), snapshot (stub) |
| `wtf-actor/src/instance/state.rs` | InstanceState + paradigm state initialization |
| `wtf-actor/src/fsm/definition.rs` | FsmDefinition: transitions + terminal states |
| `wtf-actor/src/dag/mod.rs` | DagActorState: node HashMap |
| `wtf-actor/src/procedural/context.rs` | WorkflowContext: timers, signals, activities, checkpoints |
| `wtf-actor/src/heartbeat.rs` | Heartbeat watcher (exists, not wired into serve) |
| `wtf-storage/src/nats.rs` | NATS connection with retry |
| `wtf-storage/src/journal.rs` | Event journal append |
| `wtf-storage/src/replay.rs` | Replay consumer |
| `wtf-storage/src/kv.rs` | KV buckets + heartbeat/timer operations |
| `wtf-storage/src/snapshots.rs` | Sled snapshot write/read |
| `wtf-worker/src/worker.rs` | Worker SDK: pull, dispatch, ack/nak, retry |
| `wtf-worker/src/queue.rs` | Work queue: enqueue + consumer |
| `wtf-worker/src/timer/loop.rs` | Timer poll + fire loop |
