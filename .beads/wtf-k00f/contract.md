# Contract Specification: E2E Test for Terminate Workflow

## Context

- **Feature:** End-to-end test for the workflow termination path -- from `DELETE /api/v1/workflows/:id` through the actor system to JetStream persistence and actor stop.
- **Bead:** `wtf-k00f`
- **Test file:** `crates/wtf-actor/tests/terminate_e2e.rs` (actor-level integration test, NOT HTTP-level)

### Domain Terms

| Term | Definition |
|------|-----------|
| `OrchestratorMsg::Terminate` | RPC sent to MasterOrchestrator requesting instance cancellation. Contains `instance_id`, `reason`, `reply` port. |
| `InstanceMsg::Cancel` | RPC forwarded from orchestrator to WorkflowInstance actor. Contains `reason`, `reply` port. |
| `WorkflowEvent::InstanceCancelled { reason }` | The durable event published to JetStream before actor stop. `reason` is `"api-terminate"` for HTTP-initiated termination. |
| `TerminateError::NotFound(InstanceId)` | Returned when the instance is not registered in `OrchestratorState.active`. |
| `TerminateError::Timeout(InstanceId)` | Returned when `call_cancel` exceeds `INSTANCE_CALL_TIMEOUT` (5s). |
| `GetStatusError::ActorDied` | Returned when the instance actor has stopped. Maps to HTTP 404. |
| `ACTOR_CALL_TIMEOUT` | 5 seconds -- HTTP handler timeout for orchestrator RPC. |
| `INSTANCE_CALL_TIMEOUT` | 5 seconds -- orchestrator timeout for instance actor RPC. |

### Assumptions

1. NATS server is running in Docker container `wtf-nats-test` on `localhost:4222`.
2. JetStream stream `wtf-events` is provisioned via `provision_streams()` before any operations.
3. Test operates at the **actor level** -- uses `OrchestratorMsg::Terminate` directly, NOT the HTTP layer.
4. Real `NatsClient` is used as `EventStore` -- no mocks (this is E2E).
5. A procedural `WorkflowFn` with a long `tokio::time::sleep(60s)` keeps the instance alive during cancel.
6. `provision_streams()` must be called before JetStream operations.

### Open Questions

None -- all questions resolved in `spec.md` `resolved_clarifications`.

---

## Preconditions

- **P-1:** NATS server is reachable on `localhost:4222`.
- **P-2:** JetStream stream `wtf-events` is provisioned.
- **P-3:** `NatsClient` is connected and implements `EventStore` trait.
- **P-4:** `MasterOrchestrator` actor is spawned with `OrchestratorConfig` containing `event_store: Some(Arc::new(nats_client))`.
- **P-5:** A procedural `WorkflowFn` is registered in the orchestrator's workflow registry (e.g. `"e2e-terminate-test"`).
- **P-6:** A `WorkflowInstance` is started via `OrchestratorMsg::StartWorkflow` and has entered the `Live` phase.
- **P-7:** The instance's `InstanceId` is present in `OrchestratorState.active` (i.e. `state.get(&instance_id)` returns `Some`).

## Postconditions

### Successful Termination (Happy Path)

- **PO-1:** `WorkflowEvent::InstanceCancelled { reason: "api-terminate" }` is published to JetStream subject `wtf.log.<namespace>.<instance_id>` BEFORE `myself_ref.stop()` is called.
- **PO-2:** The `handle_cancel` handler replies `Ok(())` on the RPC reply port before the actor stops.
- **PO-3:** The `WorkflowInstance` actor calls `myself_ref.stop(Some(reason))` and the actor's `post_stop` hook fires, aborting `procedural_task` and `live_subscription_task`.
- **PO-4:** The `MasterOrchestrator`'s supervision link detects actor death and calls `state.deregister(&instance_id)`, removing the instance from `active`.
- **PO-5:** A subsequent `OrchestratorMsg::GetStatus` for the terminated instance returns `Err(GetStatusError::ActorDied)` (which maps to HTTP 404).

### Failed Termination (Error Paths)

- **PO-E1:** If the instance is not in `active`, `handle_terminate` replies `Err(TerminateError::NotFound(id))`.
- **PO-E2:** If `call_cancel` times out (exceeds `INSTANCE_CALL_TIMEOUT` of 5s), `handle_terminate` replies `Err(TerminateError::Timeout(id))`.
- **PO-E3:** If the instance actor is already stopped when `Cancel` arrives, `actor_ref.call()` returns `SenderError`, which is mapped to `TerminateError::NotFound`.

### Invalid Input (HTTP-level, for reference)

- **PO-I1:** A `DELETE` path without a `/` separator returns HTTP 400 with `ApiError { error: "invalid_id", message: "bad id" }`.

## Invariants

- **I-1 (Event-before-stop):** `InstanceCancelled` is ALWAYS published to JetStream before `myself_ref.stop()` is called. (Enforced by code order in `handle_cancel`: publish at line 209, reply at line 222, stop at line 223.)
- **I-2 (Reply-before-stop):** The `Cancel` RPC reply `Ok(())` is ALWAYS sent before `myself_ref.stop()`. (Line 222 then 223.)
- **I-3 (Reason consistency):** The `reason` string in `WorkflowEvent::InstanceCancelled` matches the `reason` passed to `OrchestratorMsg::Terminate`. For HTTP-initiated termination, this is always `"api-terminate"`.
- **I-4 (Journal ordering):** When replayed via `open_replay_stream`, journal entries are returned in ascending `seq` order.
- **I-5 (No unwrap in path):** The entire terminate chain from `handle_terminate` through `call_cancel` through `handle_cancel` uses only `match`/`map_err` -- no `unwrap()` or `expect()`.
- **I-6 (Idempotent-ish behavior):** A second `Terminate` on a stopping/dead instance returns either `Ok(())` (if the first Cancel reply was already sent) or `TerminateError::NotFound`. It NEVER panics or hangs.

## Error Taxonomy

| Error Variant | When Raised | Actor-Level Result | HTTP Mapping |
|---|---|---|---|
| `TerminateError::NotFound(InstanceId)` | Instance not in `OrchestratorState.active` or actor already dead | `Err(TerminateError::NotFound)` | 404 `ApiError { error: "not_found", message: <id> }` |
| `TerminateError::Timeout(InstanceId)` | `call_cancel` exceeds `INSTANCE_CALL_TIMEOUT` (5s) | `Err(TerminateError::Timeout)` | 503 `ApiError { error: "instance_timeout", message: "cancel timed out: <id>" }` + `Retry-After: 5` |
| `GetStatusError::ActorDied` | Instance actor stopped (after cancel) | `Err(GetStatusError::ActorDied)` | 404 `ApiError { error: "actor_died", message: "instance actor is dead" }` |
| `CallResult::SenderError` | Instance actor mailbox closed during `call_cancel` | Mapped to `TerminateError::NotFound` | 404 (same as NotFound) |
| `MessagingErr::ChannelClosed` | MasterOrchestrator channel closed | Falls through to `map_actor_error` | 503 `ApiError { error: "channel_closed" }` + `Retry-After: 5` |

## Contract Signatures

```rust
// Orchestrator-level terminate (test calls this directly)
async fn handle_terminate(
    state: &mut OrchestratorState,
    instance_id: InstanceId,
    reason: String,
    reply: RpcReplyPort<Result<(), TerminateError>>,
) // -> ()

// Instance-level cancel (called internally by handle_terminate -> call_cancel)
async fn handle_cancel(
    myself_ref: ActorRef<InstanceMsg>,
    state: &InstanceState,
    reason: String,
    reply: RpcReplyPort<Result<(), WtfError>>,
) -> Result<(), ActorProcessingErr>

// JetStream publish (called internally by handle_cancel)
fn publish(
    &self,
    namespace: &NamespaceId,
    instance_id: &InstanceId,
    event: WorkflowEvent,
) -> impl Future<Output = Result<u64, WtfError>>

// Journal replay (test uses this to verify InstanceCancelled)
fn open_replay_stream(
    &self,
    namespace: &NamespaceId,
    instance_id: &InstanceId,
    from_seq: u64,
) -> impl Future<Output = Result<Box<dyn ReplayStream>, WtfError>>

// Status check (test uses this to verify actor is dead)
// OrchestratorMsg::GetStatus { instance_id, reply }
// reply receives: Result<Option<InstanceStatusSnapshot>, GetStatusError>
```

## HTTP Contract Summary (for reference only -- test is actor-level)

| Method | Path | Success | Not Found | Invalid Input | Timeout |
|--------|------|---------|-----------|---------------|---------|
| `DELETE` | `/api/v1/workflows/<ns>/<id>` | 204 No Content | 404 `ApiError { error: "not_found" }` | 400 `ApiError { error: "invalid_id" }` | 503 `ApiError { error: "instance_timeout" }` + `Retry-After: 5` |

## Non-goals

- **NOT testing HTTP routing** -- the test operates at the actor message level.
- **NOT testing `DELETE` with no slash** -- that is an HTTP-level path parsing concern.
- **NOT testing concurrent double-terminate** -- covered as an edge case but not the primary focus.
- **NOT testing crash recovery** -- terminate is a clean shutdown; recovery is a separate concern.
- **NOT testing the axum `terminate_workflow` handler directly** -- the handler is a thin wrapper that delegates to `map_terminate_result`.
