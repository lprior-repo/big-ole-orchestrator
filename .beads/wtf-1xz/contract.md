# Contract Specification

## Context
- Feature: Implement `send_signal` handler for Bead wtf-1xz per ADR-012.
- Domain terms:
    - `instance_id`: Unique identifier for a workflow instance.
    - `signal_name`: The identifier for the event/signal being sent.
    - `payload`: Data associated with the signal (JSON).
    - `ActorRef<OrchestratorMsg>`: Reference to the master orchestrator actor (Ractor).
    - `WtfError`: Domain error type for wtf-engine.
- Assumptions:
    - The `id` Path parameter is expected to be in `<namespace>/<id>` format based on `split_path_id`.
    - `split_path_id` only validates the presence of `/`; it does NOT validate that the `id` portion is a valid ULID.
    - The orchestrator handles the routing of signals to the specific workflow instance.
    - `ACTOR_CALL_TIMEOUT` is the standard timeout for actor RPC calls.
- Open questions:
    - None (Requirements strictly defined by user).

## Preconditions
- [ ] `master` ActorRef must be valid and reachable.
- [ ] `id` must be a path string with at least one `/` that `split_path_id` can parse into `(namespace, InstanceId)`. Note: `InstanceId` accepts any string; no ULID validation is performed.
- [ ] `req.payload` must be serializable to JSON bytes.

## Postconditions
- [ ] On success, returns `202 Accepted` status code.
- [ ] Success response body MUST be `{"acknowledged": true}`.
- [ ] The `OrchestratorMsg::Signal` must be sent to the master actor with correct `instance_id`, `signal_name`, and `payload`.

## Invariants
- [ ] The system state remains consistent regardless of whether the signal is successfully delivered to the specific instance (at the API layer).

## Error Taxonomy
- `ApiError::InvalidId` (400 Bad Request) - when `id` path is malformed (e.g., missing slash).
- `ApiError::InvalidPayload` (400 Bad Request) - when `V3SignalRequest` payload cannot be serialized to bytes or is malformed.
- `ApiError::InstanceNotFound` (404 Not Found) - when the orchestrator returns `Ok(CallResult::Success(Err(WtfError::InstanceNotFound)))`.
- `ApiError::ActorTimeout` (500 Internal Server Error) - when the actor call times out (`ACTOR_CALL_TIMEOUT`).
- `ApiError::ActorError` (500 Internal Server Error) - for other actor communication failures (MessagingErr, CallResult::Error).

## Response Shapes
### Success (202 Accepted)
```json
{
  "acknowledged": true
}
```

### Error (4xx/5xx)
```json
{
  "error": "string_code",
  "message": "human_readable_message"
}
```

## Mocking Strategy
- **Tool**: `ractor::mock` or a custom manual mock of `ActorRef<OrchestratorMsg>`.
- **Approach**:
    1. Define a mock orchestrator that accepts `OrchestratorMsg::Signal`.
    2. Use `master.call` and verify the received message fields (`instance_id`, `signal_name`, `payload`).
    3. Program the mock to return `Ok(Ok(()))`, `Ok(Err(WtfError::InstanceNotFound))`, `Ok(CallResult::Timeout)`, or `Err(MessagingErr)`.
    4. Assert that the HTTP response matches the expected status code and body for each mock behavior.

## Contract Signatures
- `pub async fn send_signal(Extension(master): Extension<ActorRef<OrchestratorMsg>>, Path(id): Path<String>, Json(req): Json<V3SignalRequest>) -> impl IntoResponse`
- `fn map_signal_result(res: Result<CallResult<Result<(), WtfError>>, MessagingErr<OrchestratorMsg>>) -> impl IntoResponse`

## Non-goals
- [ ] Validating the internal structure of the `payload` against a workflow-specific schema.
- [ ] Implementing the signal routing logic within the Orchestrator actor.
