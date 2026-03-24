# Contract Specification: E2E Signal Delivery Test

## Context

- **Feature:** End-to-end integration test validating signal delivery through the actor message pipeline
- **Bead:** wtf-h8u4
- **Test file:** `crates/wtf-actor/tests/signal_delivery_e2e.rs`

### Domain Terms

| Term | Definition |
|------|-----------|
| `OrchestratorMsg::Signal` | RPC sent to MasterOrchestrator to deliver a signal to a running instance |
| `InstanceMsg::InjectSignal` | Internal message from master to WorkflowInstance carrying signal_name + payload |
| `handle_signal` | Instance handler: persists SignalReceived event, delivers to pending waiter OR buffers |
| `handle_wait_for_signal` | Instance handler: checks buffer first, then registers RPC port in `pending_signal_calls` |
| `WorkflowContext::wait_for_signal` | Dual-phase API on procedural context: checkpoint check, then RPC to instance actor |
| `pending_signal_calls` | `HashMap<String, RpcReplyPort<Result<Bytes, WtfError>>>` on InstanceState |
| `received_signals` | `HashMap<String, Vec<Bytes>>` on ProceduralActorState — FIFO buffer for early-arriving signals |
| `MockEventStore` | Test double returning `Ok(1)` from publish, `EmptyReplayStream` from replay |
| `SignalReceived` | `WorkflowEvent` variant: `{ signal_name: String, payload: Bytes }` |

### Source Grounding

All contract statements are grounded in the actual codebase:

1. **HTTP endpoint:** `POST /api/v1/workflows/:id/signals` (app.rs:58, signal.rs:15)
2. **HTTP request:** `V3SignalRequest { signal_name: String, payload: serde_json::Value }` (requests.rs:40-43)
3. **HTTP response:** `SignalResponse { acknowledged: bool }` with status 202 (responses.rs:48, signal.rs:61-63)
4. **Master handler:** `master/handlers/signal.rs:8-27` — looks up instance ref, sends `InstanceMsg::InjectSignal` via cast, returns `Err(InstanceNotFound)` if missing
5. **Instance handler:** `instance/handlers.rs:136-179` (`handle_signal`) — publishes SignalReceived event, then EITHER delivers to pending RPC port OR buffers in `received_signals`
6. **Wait handler:** `instance/procedural.rs:97-128` (`handle_wait_for_signal`) — checks `received_signals` buffer first, then registers in `pending_signal_calls`
7. **Context API:** `procedural/context.rs:195-240` (`wait_for_signal`) — dual-phase: checkpoint replay then live RPC
8. **Event injection wake:** `instance/handlers.rs:123-131` — `handle_inject_event_msg` matches `SignalReceived`, sends payload to pending RPC port

### Assumptions

- Beads wtf-88f4 (pending_signal_calls), wtf-3cv7 (wait_for_signal), wtf-cedw (SignalReceived persistence + wake) are ALREADY implemented (codebase confirms: all three exist)
- `MockEventStore` returning `Ok(1)` from publish is sufficient for actor to proceed with event injection
- Test does NOT require a real NATS server (proven by spawn_workflow_test.rs pattern)

### Open Questions

None — all implementation code is present and readable.

---

## Preconditions

### For the test infrastructure

- **PRE-1:** `MockEventStore` implements `EventStore` trait (publish returns `Ok(1)`, open_replay_stream returns `EmptyReplayStream`)
- **PRE-2:** `EmptyReplayStream` implements `ReplayStream` trait (next_event returns `TailReached`, next_live_event hangs forever)
- **PRE-3:** `OrchestratorConfig` is constructed with `procedural_workflow: Some(Arc::new(SignalWorkflowFn))` and `event_store: Some(Arc::new(MockEventStore))`
- **PRE-4:** `MasterOrchestrator` is spawned and reachable via `ActorRef<OrchestratorMsg>`
- **PRE-5:** Test runs with `--test-threads=1` (no shared mutable state between tests)

### For individual test scenarios

- **PRE-6:** A procedural workflow instance has been started via `OrchestratorMsg::StartWorkflow` with a known instance_id
- **PRE-7:** The WorkflowFn calls `ctx.wait_for_signal(signal_name).await` at some point in its execution
- **PRE-8:** `InstanceState.pending_signal_calls` is empty before the signal (for the "waiter present" path)
- **PRE-9:** `InstanceState.pending_signal_calls` is empty AND the signal arrives before `wait_for_signal` registers (for the "early signal" path)

## Postconditions

### Signal delivery (happy path)

- **POST-1:** `OrchestratorMsg::Signal` RPC returns `Ok(())`
- **POST-2:** `WorkflowEvent::SignalReceived` is published to the EventStore (seq returned > 0)
- **POST-3:** The event is injected into paradigm state via `inject_event`
- **POST-4:** `pending_signal_calls` entry for the signal_name is removed (waiter woken)
- **POST-5:** `WorkflowContext::wait_for_signal` returns `Ok(payload)` where `payload` matches what was sent
- **POST-6:** The WorkflowFn completes (returns `Ok(())`) after receiving the signal
- **POST-7:** `ProceduralWorkflowCompleted` is sent to the instance actor
- **POST-8:** The instance stops (no longer queryable via GetStatus)

### Signal buffering (early arrival)

- **POST-9:** When signal arrives before waiter: signal is buffered in `received_signals` HashMap
- **POST-10:** When `wait_for_signal` is subsequently called: buffered signal is consumed from `received_signals` immediately
- **POST-11:** The consumed buffer entry is removed if the Vec is now empty
- **POST-12:** `wait_for_signal` returns `Ok(payload)` without blocking

### Error paths

- **POST-13:** Signal to nonexistent instance: `OrchestratorMsg::Signal` returns `Err(WtfError::InstanceNotFound { .. })`
- **POST-14:** Signal with wrong name to a waiting workflow: workflow remains blocked; signal is buffered in `received_signals` under the wrong name (not discarded)

### Edge cases

- **POST-15:** Empty signal payload (`Bytes::new()`): `wait_for_signal` returns `Ok(Bytes::new())` and workflow completes

## Invariants

- **INV-1:** `op_counter` increments exactly once per `wait_for_signal` call (fetched at entry via `SeqCst`, incremented at return)
- **INV-2:** Signal payload delivered to `wait_for_signal` exactly matches the payload sent via `OrchestratorMsg::Signal`
- **INV-3:** `pending_signal_calls` contains at most one entry per signal_name at any time
- **INV-4:** A signal is never lost: it is either delivered to a pending waiter OR buffered in `received_signals`
- **INV-5:** `received_signals` preserves FIFO ordering for multiple arrivals of the same signal_name
- **INV-6:** `total_events_applied` increments by exactly 1 for the `SignalReceived` event
- **INV-7:** `ProceduralActorState.operation_counter` increments by exactly 1 when `SignalReceived` is applied via `apply_event` (apply.rs:198-212)

## Error Taxonomy

| Error Variant | When It Occurs | Source |
|---------------|----------------|--------|
| `WtfError::InstanceNotFound { instance_id }` | Signal sent to instance_id not registered in orchestrator state | `master/handlers/signal.rs:24` |
| `WtfError::NatsPublish { .. }` | EventStore.publish fails when persisting SignalReceived | `instance/handlers.rs:173-175` |
| `WtfError::NatsPublish { .. }` (missing store) | EventStore is None on InstanceState | `instance/handlers.rs:145-146` |
| `anyhow::Error` ("Actor call failed") | Ractor RPC returns non-Success variant | `context.rs:238` |
| `ractor::rpc::CallResult::Timeout` | RPC call exceeds timeout duration | N/A in this test (no timeout set on wait_for_signal) |

## Contract Signatures

### RPC under test

```rust
// MasterOrchestrator RPC
OrchestratorMsg::Signal {
    instance_id: InstanceId,
    signal_name: String,
    payload: Bytes,
    reply: RpcReplyPort<Result<(), WtfError>>,
}

// Internal instance message (cast, not RPC)
InstanceMsg::InjectSignal {
    signal_name: String,
    payload: Bytes,
    reply: RpcReplyPort<Result<(), WtfError>>,
}

// Internal instance RPC (workflow context waiting)
InstanceMsg::ProceduralWaitForSignal {
    operation_id: u32,
    signal_name: String,
    reply: RpcReplyPort<Result<Bytes, WtfError>>,
}
```

### WorkflowContext API under test

```rust
impl WorkflowContext {
    pub async fn wait_for_signal(&self, signal_name: &str) -> anyhow::Result<Bytes>;
}
```

### Test helper signatures

```rust
async fn send_signal_rpc(
    orchestrator: &ActorRef<OrchestratorMsg>,
    instance_id: &str,
    signal_name: &str,
    payload: Bytes,
) -> Result<(), WtfError>;

async fn start_workflow_rpc(
    orchestrator: &ActorRef<OrchestratorMsg>,
    instance_id: &str,
) -> Result<InstanceId, StartError>;

async fn get_status_rpc(
    orchestrator: &ActorRef<OrchestratorMsg>,
    instance_id: &str,
) -> Option<InstanceStatusSnapshot>;
```

### Test WorkflowFn

```rust
#[async_trait]
impl WorkflowFn for SignalWorkflowFn {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()>;
}
```

## Non-goals

- HTTP-level signal tests (covered by `crates/wtf-api/src/handlers/signal.rs` tests)
- NATS JetStream durability tests (separate integration test with real NATS)
- Signal delivery during crash recovery / event replay
- Multiple waiters for the same signal name
- Signal ordering across multiple signal names
- Signal with large payloads (multi-MB)
- Workflow cancellation while waiting for a signal
