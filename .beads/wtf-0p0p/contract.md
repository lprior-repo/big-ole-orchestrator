# Contract Specification: epic: Phase 2 — Actor Core (wtf-actor)

## Overview

Ractor-based execution engine with MasterOrchestrator root supervisor, three WorkflowInstance actor variants (FsmActor, DagActor, ProceduralActor), snapshot trigger, heartbeat-driven crash recovery. All actors derive state exclusively from JetStream replay — zero durable state in memory across restarts.

## Related ADRs

- ADR-003: Actor supervision model
- ADR-006: JetStream state persistence
- ADR-015: Snapshot strategy
- ADR-016: Heartbeat protocol
- ADR-017: Crash recovery flow
- ADR-019: WorkflowInstance actor variants

## Core Components

### 1. MasterOrchestrator

**Responsibilities:**
- Root supervisor for all workflow actors
- Spawns and supervises FsmActor, DagActor, ProceduralActor
- Routes workflow commands to appropriate actor variant
- Manages actor lifecycle (spawn, supervise, terminate)

**Preconditions:**
- JetStream connection must be established
- Phase 1 dependencies must be available

**Postconditions:**
- All child actors are supervised and respawned on failure
- Workflow state is durably persisted via JetStream

**Error Taxonomy:**
- `OrchestratorError::SpawnFailed` - Actor spawn failure
- `OrchestratorError::JetStreamUnavailable` - JetStream connection lost
- `OrchestratorError::WorkflowNotFound` - Unknown workflow ID

### 2. FsmActor (Finite State Machine Workflow)

**Responsibilities:**
- Executes FSM-based workflows with explicit states and transitions
- Validates transition eligibility before state changes
- Emits state change events to JetStream

**Preconditions:**
- Workflow definition with valid FSM graph
- Initial state must be defined

**Postconditions:**
- Current state is always derivable from JetStream replay
- All transitions are logged for replay

**State Machine Invariants:**
- Current state ∈ valid_states
- Transition(t) only valid if t ∈ valid_transitions(current_state)
- No mutable state survives restart (derived from JetStream)

### 3. DagActor (Directed Acyclic Graph Workflow)

**Responsibilities:**
- Executes DAG-based workflows with task dependencies
- Schedules tasks when all dependencies complete
- Handles task failure with proper DAG semantics

**Preconditions:**
- Workflow definition with valid DAG (no cycles)
- All task dependencies must be resolvable

**Postconditions:**
- All completed tasks are durably logged
- Failed tasks trigger proper DAG re-computation on restart

**DAG Invariants:**
- No cycles in task graph
- Task status ∈ {Pending, Running, Completed, Failed}
- Completed tasks are never re-run unless upstream fails

### 4. ProceduralActor (Procedural Script Workflow)

**Responsibilities:**
- Executes step-by-step procedural workflows
- Supports pause/resume semantics
- Maintains step cursor durably

**Preconditions:**
- Workflow definition with ordered steps
- Step index must be valid

**Postconditions:**
- Current step index is durable
- Completed steps are never re-executed

**Procedural Invariants:**
- Current step index ∈ [0, total_steps]
- Step n only executes after step n-1 completes
- No mutable cursor survives restart

### 5. Snapshot Trigger

**Responsibilities:**
- Monitors actor state changes
- Triggers snapshots at configurable intervals
- Prunes old snapshots beyond retention window

**Preconditions:**
- Actor must have made state modifications

**Postconditions:**
- Snapshot is written to JetStream
- Old snapshots are garbage collected

### 6. Heartbeat-Driven Crash Recovery

**Responsibilities:**
- Actors emit periodic heartbeats
- Supervisor detects missed heartbeats
- Triggers recovery via JetStream replay

**Preconditions:**
- Heartbeat interval must be configured
- JetStream must be reachable

**Postconditions:**
- Failed actor is respawned
- Actor state is reconstructed from JetStream replay
- No state loss beyond last snapshot

## Ownership Contracts

| Actor | Receives | Sends |
|-------|----------|-------|
| MasterOrchestrator | WorkflowCommand, SpawnCommand | ActorRef, SupervisionEvent |
| FsmActor | FsmCommand, StateTransitionEvent | FsmEvent, SnapshotTrigger |
| DagActor | DagCommand, TaskEvent | DagEvent, SnapshotTrigger |
| ProceduralActor | ProceduralCommand, StepEvent | ProceduralEvent, SnapshotTrigger |
| SnapshotTrigger | StateChangeEvent | SnapshotCommand |
| HeartbeatMonitor | HeartbeatEvent | RecoveryCommand |

## Type-Encoded Preconditions

```rust
// MasterOrchestrator spawn precondition
fn can_spawn(actor_type: ActorType) -> bool {
    matches!(actor_type, Fsm | Dag | Procedural)
}

// FsmActor transition precondition  
fn can_transition(current: State, transition: Transition) -> bool {
    valid_transitions(current).contains(&transition)
}

// DagActor task execution precondition
fn can_execute_task(task: TaskId, dag: &Dag) -> bool {
    dag.dependencies(task).all(|dep| dag.is_completed(dep))
}

// ProceduralActor step precondition
fn can_execute_step(step: StepIndex, total: StepIndex) -> bool {
    step < total
}
```

## Violation Examples

1. **Invalid FSM Transition**: Attempting transition from State::Running to State::Completed when only State::Running → State::Paused is valid
2. **DAG Cycle**: Workflow definition contains cycle A → B → C → A
3. **Step Overflow**: Procedural workflow with 5 steps receives command to execute step 5 (0-indexed max is 4)
4. **Orphaned Actor**: Actor spawned without JetStream connection cannot persist state

## JetStream Message Contracts

| Subject | Payload |
|---------|---------|
| `workflow.{id}.fsm.event` | FsmEvent (state, transition, timestamp) |
| `workflow.{id}.dag.event` | DagEvent (task_id, status, timestamp) |
| `workflow.{id}.procedural.event` | ProceduralEvent (step, status, timestamp) |
| `workflow.{id}.snapshot` | Snapshot (actor_state, timestamp) |
| `workflow.{id}.heartbeat` | Heartbeat (actor_id, timestamp) |
