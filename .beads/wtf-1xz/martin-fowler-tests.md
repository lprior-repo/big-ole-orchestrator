# Martin Fowler Test Plan

## Happy Path Tests
- `test_returns_success_when_valid_input_provided`
    - **Given**: A valid `id` path "ns/id", a valid `signal_name`, and a valid JSON `payload`.
    - **When**: `send_signal` is called.
    - **Then**: Returns `202 Accepted` with body `{"acknowledged": true}` AND `OrchestratorMsg::Signal` is sent to the master actor with correct values.

## Error Path Tests
- `test_returns_bad_request_when_invalid_id_format`
    - **Given**: An `id` path missing a slash ("invalidid").
    - **When**: `send_signal` is called.
    - **Then**: Returns `400 Bad Request` with error code "invalid_id" and descriptive message.
- `test_returns_bad_request_when_payload_malformed`
    - **Given**: A request body with a `payload` that fails `serde_json::to_vec`.
    - **When**: `send_signal` attempts to serialize the payload to bytes.
    - **Then**: Returns `400 Bad Request` with error code "invalid_payload".
- `test_returns_not_found_when_workflow_instance_missing`
    - **Given**: A valid `id` path but the instance does not exist in the system.
    - **When**: Orchestrator returns `Ok(CallResult::Success(Err(VoError::InstanceNotFound)))`.
    - **Then**: Returns `404 Not Found` with error code "instance_not_found".
- `test_returns_internal_server_error_on_actor_timeout`
    - **Given**: The master actor does not respond within `ACTOR_CALL_TIMEOUT`.
    - **When**: Orchestrator returns `Ok(CallResult::Timeout)`.
    - **Then**: Returns `500 Internal Server Error` with error code "actor_timeout".
- `test_returns_internal_server_error_on_actor_failure`
    - **Given**: The master actor's mailbox is full or the actor has panicked.
    - **When**: `master.call` returns `Err(MessagingErr)` or `Ok(CallResult::Error)`.
    - **Then**: Returns `500 Internal Server Error` with error code "actor_error".

## Edge Case Tests
- `test_handles_null_payload_gracefully`
    - **Given**: A `V3SignalRequest` with `payload: null`.
    - **When**: `send_signal` is called.
    - **Then**: Returns `202 Accepted` and forwards `null` (as JSON bytes) to the actor.

## Contract Verification Tests
- `test_precondition_valid_id`
    - Ensures that any malformed ID results in a short-circuit before calling the orchestrator.
- `test_postcondition_actor_message_sent`
    - Verifies that `OrchestratorMsg::Signal` is actually called on the master actor with correct `instance_id`, `signal_name`, and serialized `payload`.

## Given-When-Then Scenarios

### Scenario 1: Successfully sending a signal
Given:
- A running master orchestrator actor (mocked).
- A valid workflow ID "default/01ARZ3NDEKTSV4RRFFQ69G5FAV".
- A SignalRequest with signal_name "payment_approved" and a JSON payload `{"approved": true}`.
When:
- `send_signal` is called with these parameters.
Then:
- The orchestrator MUST receive `OrchestratorMsg::Signal` with:
    - `instance_id`: "01ARZ3NDEKTSV4RRFFQ69G5FAV"
    - `signal_name`: "payment_approved"
    - `payload`: Bytes matching `{"approved": true}`
- The response status MUST be `202 Accepted`.
- The response body MUST be `{"acknowledged": true}`.

### Scenario 2: Sending a signal to a non-existent workflow
Given:
- A running master orchestrator actor (mocked).
- A workflow ID "default/missing-id".
- The mock is programmed to return `Ok(CallResult::Success(Err(VoError::InstanceNotFound)))`.
When:
- `send_signal` is called.
Then:
- The response status MUST be `404 Not Found`.
- The error response body MUST contain code "instance_not_found".

### Scenario 3: Actor timeout during signal delivery
Given:
- A running master orchestrator actor (mocked).
- The mock is programmed to return `Ok(CallResult::Timeout)`.
When:
- `send_signal` is called.
Then:
- The response status MUST be `500 Internal Server Error`.
- The error response body MUST contain code "actor_timeout".

### Scenario 4: Malformed ID format
Given:
- An ID string "no-slash-here".
When:
- `send_signal` is called.
Then:
- The orchestrator actor MUST NOT be called.
- The response status MUST be `400 Bad Request`.
- The error response body MUST contain code "invalid_id".
