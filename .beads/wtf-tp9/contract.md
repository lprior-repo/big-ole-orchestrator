bead_id: wtf-tp9
bead_title: "bead: Implement terminate_workflow handler"
phase: "STATE 1"
updated_at: "2026-03-20T22:37:00Z"

## Context
- Feature: Implement terminate_workflow handler per ADR-012
- Domain terms:
  - `terminate_workflow`: Axum handler that terminates a running workflow by invocation_id
  - `OrchestratorMsg::Terminate`: Message sent to the orchestrator actor
  - `ActorRef<OrchestratorMsg>`: Reference to the orchestrator actor
  - `invocation_id`: Unique identifier for a workflow invocation
- Location: wtf-api/src/handlers.rs
- Assumptions:
  - The OrchestratorMsg enum has a Terminate variant
  - ActorRef<OrchestratorMsg> is available via Extension layer
  - Extension<ActorRef<OrchestratorMsg>> provides access to the master actor

## Preconditions
- P1: `invocation_id` path parameter must be a non-empty string
- P2: The `master` ActorRef must be available in Extension (no channel failure)
- P3: The workflow with given invocation_id must exist (or 404 returned)

## Postconditions
- Q1: On success: HTTP 204 No Content is returned
- Q2: On workflow not found: HTTP 404 is returned
- Q3: On actor communication failure: HTTP 500 Internal Server Error is returned

## Invariants
- I1: No partial state changes - either workflow is terminated or error is returned
- I2: terminate operation is idempotent (calling twice with same id returns same result)

## Error Taxonomy
- Error::WorkflowNotFound { invocation_id } - when workflow doesn't exist → 404
- Error::ActorCommunicationFailed - when call to orchestrator fails → 500
- Error::InvalidInvocationId - when invocation_id is malformed/empty → 400

## Contract Signatures
```rust
pub async fn terminate_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(invocation_id): Path<String>,
) -> Result<StatusCode, StatusCode>
```

## Type Encoding
| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| invocation_id non-empty | Runtime-checked | String length check |
| master available | Compile-time | Extension<ActorRef<...>> |
| workflow exists | Runtime-checked | Result error variant |

## Violation Examples
- VIOLATES P1: `terminate_workflow(Extension(...), Path(""))` → `Err(400 BadRequest)`
- VIOLATES P2: If Extension is missing → compile-time error (not possible)
- VIOLATES P3: `terminate_workflow(Extension(...), Path("nonexistent-id"))` → `Err(404 NotFound)`
