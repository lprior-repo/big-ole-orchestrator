# Martin Fowler Tests: epic: Phase 2 — Actor Core (wtf-actor)

## Test Strategy

Given-When-Then format for BDD-style specification. Each test represents a behavioral scenario verified through JetStream replay verification.

---

## MasterOrchestrator Tests

### Test: Orchestrator spawns FsmActor successfully

**Given:** JetStream connection is established, no existing FsmActor for workflow-123
**When:** `SpawnCommand { workflow_id: "workflow-123", actor_type: Fsm }` is received
**Then:** FsmActor is spawned, `ActorRef { workflow_id: "workflow-123", actor_type: Fsm }` is returned

### Test: Orchestrator respawns actor on heartbeat failure

**Given:** FsmActor for workflow-123 is running, heartbeat fails for 3 consecutive intervals
**When:** HeartbeatMonitor detects failure
**Then:** FsmActor is terminated, new FsmActor is spawned, state is replayed from JetStream

### Test: Orchestrator rejects invalid actor type

**Given:** Valid JetStream connection
**When:** `SpawnCommand { actor_type: Unknown }` is received
**Then:** `OrchestratorError::InvalidActorType` is returned, no actor spawned

---

## FsmActor Tests

### Test: FsmActor transitions state validly

**Given:** FsmActor with workflow-123, current_state = State::Running, valid transitions = {Running → Paused, Running → Completed}
**When:** `FsmCommand::Transition { transition: Transition::RunningToPaused }` is received
**Then:** state becomes State::Paused, `FsmEvent::StateChanged { from: Running, to: Paused }` is emitted to JetStream

### Test: FsmActor rejects invalid transition

**Given:** FsmActor with workflow-123, current_state = State::Running, valid transitions = {Running → Paused}
**When:** `FsmCommand::Transition { transition: Transition::RunningToCompleted }` is received
**Then:** state remains Running (same reference, no state mutation), `FsmEvent::TransitionRejected { reason: InvalidTransition }` is emitted, no event written to JetStream, command returns error result

### Test: FsmActor recovers state from JetStream replay

**Given:** FsmActor crashes after emitting 5 state changes to JetStream
**When:** New FsmActor is spawned with same workflow_id
**Then:** Initial state is reconstructed from JetStream replay, matches last persisted state

### Test: FsmActor rejects transition in terminal state

**Given:** FsmActor with workflow-123, current_state = State::Completed (terminal)
**When:** Any `FsmCommand::Transition` is received
**Then:** `FsmEvent::TransitionRejected { reason: TerminalState }` is emitted, state remains Completed (same reference), no event written to JetStream, command returns error

### Test: FsmActor handles JetStream unavailable during command

**Given:** FsmActor with workflow-123, current_state = Running, JetStream connection is lost
**When:** `FsmCommand::Transition { transition: Transition::RunningToPaused }` is received
**Then:** `OrchestratorError::JetStreamUnavailable` is returned, state remains Running (same reference), no event emitted to JetStream

### Test: FsmActor handles unknown workflow ID

**Given:** No FsmActor exists for workflow-999
**When:** `FsmCommand::Transition { workflow_id: "workflow-999", transition: ... }` is received
**Then:** `OrchestratorError::WorkflowNotFound` is returned, no state change occurs

---

## DagActor Tests

### Test: DagActor executes task when all dependencies complete

**Given:** DagActor with workflow-123, task-A depends on [], task-B depends on [task-A]
**When:** `DagCommand::Complete { task_id: task-A }` is received
**Then:** task-B becomes eligible for execution, `DagEvent::TaskEligible { task_id: task-B }` is emitted

### Test: DagActor marks workflow complete when all tasks done

**Given:** DagActor with workflow-123, all tasks completed except final task-C
**When:** `DagCommand::Complete { task_id: task-C }` is received
**Then:** workflow status becomes Completed, `DagEvent::WorkflowCompleted` is emitted

### Test: DagActor handles task failure with retry

**Given:** DagActor with workflow-123, task-A fails, max_retries = 3
**When:** `DagCommand::Fail { task_id: task-A, error: "timeout" }` is received
**Then:** retry_count incremented, `DagEvent::TaskRetry { task_id: task-A, attempt: 2 }` is emitted, task re-queued

### Test: DagActor fails workflow after max retries exceeded

**Given:** DagActor with workflow-123, task-A fails 3 times (max_retries = 3)
**When:** `DagCommand::Fail { task_id: task-A, error: "timeout" }` is received (3rd time)
**Then:** workflow status becomes Failed, `DagEvent::WorkflowFailed { reason: MaxRetriesExceeded }` is emitted

### Test: DagActor detects cycle in workflow definition

**Given:** Workflow definition with cycle: task-A → task-B → task-C → task-A
**When:** DagActor validates workflow on startup
**Then:** `DagError::CyclicDependency` is returned, workflow is rejected

### Test: DagActor recovers state from JetStream replay

**Given:** DagActor crashes after completing task-A and task-B
**When:** New DagActor is spawned with same workflow_id
**Then:** task-A and task-B are marked Completed from replay, workflow resumes from task-C

---

## ProceduralActor Tests

### Test: ProceduralActor executes steps sequentially

**Given:** ProceduralActor with workflow-123, total_steps = 5, current_step = 0
**When:** `ProceduralCommand::ExecuteNext` is received
**Then:** step 0 executes, `ProceduralEvent::StepCompleted { step: 0 }` is emitted, current_step becomes 1

### Test: ProceduralActor pauses at current step

**Given:** ProceduralActor with workflow-123, current_step = 2
**When:** `ProceduralCommand::Pause` is received
**Then:** current_step (2) is persisted to JetStream, actor enters Paused state

### Test: ProceduralActor resumes from persisted step

**Given:** ProceduralActor with workflow-123 crashed at step 2, step 2 persisted to JetStream
**When:** New ProceduralActor is spawned
**Then:** current_step = 2, next command resumes from step 2

### Test: ProceduralActor completes workflow at final step

**Given:** ProceduralActor with workflow-123, total_steps = 3, current_step = 2
**When:** `ProceduralCommand::ExecuteNext` is received
**Then:** step 2 executes, `ProceduralEvent::WorkflowCompleted` is emitted, workflow status = Completed

### Test: ProceduralActor rejects step beyond total

**Given:** ProceduralActor with workflow-123, total_steps = 3, current_step = 3
**When:** `ProceduralCommand::ExecuteNext` is received
**Then:** `ProceduralError::StepOutOfRange` is returned

---

## Snapshot Trigger Tests

### Test: SnapshotTrigger creates snapshot on interval

**Given:** Actor has made state modifications, snapshot_interval = 30 seconds, last_snapshot = 60 seconds ago
**When:** SnapshotTrigger evaluates actor
**Then:** New snapshot is written to JetStream, last_snapshot timestamp updated

### Test: SnapshotTrigger handles write failure

**Given:** Actor has made state modifications, snapshot_interval elapsed, JetStream write fails mid-operation
**When:** SnapshotTrigger attempts to write snapshot
**Then:** `OrchestratorError::SnapshotWriteFailed` is returned, actor continues running with in-memory state, error is logged, retry scheduled

### Test: SnapshotTrigger prunes old snapshots beyond retention

**Given:** SnapshotTrigger with retention = 5 snapshots, actor has 6 existing snapshots
**When:** New snapshot is created
**Then:** Oldest snapshot is deleted from JetStream, 5 snapshots remain

---

## Heartbeat-Driven Recovery Tests

### Test: Actor emits heartbeat periodically

**Given:** Actor running with heartbeat_interval = 5 seconds
**When:** 5 seconds elapse since last heartbeat
**Then:** `HeartbeatEvent { actor_id, timestamp }` is published to JetStream

### Test: HeartbeatMonitor detects missed heartbeat

**Given:** Actor heartbeat_interval = 5 seconds, last_heartbeat = 20 seconds ago, missed_threshold = 3
**When:** HeartbeatMonitor evaluates actor
**Then:** `RecoveryCommand::Respawn { actor_id }` is triggered

### Test: Recovery reconstructs full state from snapshot + events

**Given:** Actor crashed, last_snapshot at t=100, events at t=101, t=102, t=103
**When:** Actor is respawned and recovers
**Then:** State is reconstructed as: snapshot(t=100) + replay(t=101) + replay(t=102) + replay(t=103)

---

## JetStream Replay Invariants (All Actors)

### Test: Zero durable state in memory after restart

**Given:** Any actor with persisted state in JetStream
**When:** Actor process restarts
**Then:** No state exists in actor memory, all state must be derived from JetStream replay

### Test: All state changes are emitted to JetStream before acknowledgment

**Given:** Actor receives command that modifies state
**When:** State change occurs
**Then:** `Event` is written to JetStream and acknowledged before command returns success

### Test: Replay produces identical state

**Given:** Actor has processed events E1, E2, E3, E4
**When:** Full replay is performed from scratch
**Then:** Final state matches original state after E4

---

## Concurrency Tests

### Test: Concurrent commands to same actor are serialized

**Given:** FsmActor with workflow-123, current_state = Running
**When:** Two `FsmCommand::Transition` commands arrive simultaneously (concurrent queue)
**Then:** Commands are processed sequentially, state machine enforces mutual exclusion, no race condition occurs, each command receives ordered response

### Test: Concurrent commands preserve state consistency

**Given:** FsmActor with workflow-123, current_state = Running, valid transitions = {Running → Paused, Running → Completed}
**When:** `Transition RunningToPaused` and `Transition RunningToCompleted` arrive concurrently
**Then:** Exactly one transition succeeds, the other receives `FsmEvent::TransitionRejected { reason: InvalidTransition }`, final state is deterministic (whichever won the race)

### Test: Concurrent spawn commands for same workflow_id

**Given:** No actor exists for workflow-123
**When:** Two `SpawnCommand { workflow_id: "workflow-123" }` arrive simultaneously
**Then:** Exactly one spawn succeeds, the other receives `OrchestratorError::WorkflowAlreadyExists`, exactly one FsmActor exists for workflow-123

---

## Deferred to Integration Testing

### Test: Zero durable state in memory after restart (DEFERRED TO INTEGRATION)

**Reason:** Requires actual process restart to verify no mutable state survives. This is an integration/E2E test that cannot be performed in unit test context with mocked components.

**Verification Method:** Spin up actor, persist state to JetStream, kill process, restart process, verify actor recovers state purely from JetStream replay without any in-memory state surviving.

**Impact:** Critical - This is the core architecture guarantee. Must be verified in integration test suite before production deployment.

### Test: Snapshot write atomicity during crash (DEFERRED TO INTEGRATION)

**Reason:** Verifying atomicity of snapshot writes during process crash requires actual process termination mid-write. Cannot be reliably simulated in unit tests with mocked JetStream.

**Verification Method:** Write large snapshot, crash process mid-write, verify on restart that either full old snapshot or full new snapshot exists (no partial/corrupt snapshot).
