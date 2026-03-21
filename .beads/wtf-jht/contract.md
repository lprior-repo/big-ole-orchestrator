bead_id: wtf-jht
bead_title: "bead: Implement list_workflows handler"
phase: "STATE 1"
updated_at: "2026-03-20T23:45:00Z"

## Context
- Feature: Implement list_workflows handler per ADR-012
- Domain terms:
  - `list_workflows`: Axum handler that returns all running workflows
  - `OrchestratorMsg::ListWorkflows`: Message sent to orchestrator actor requesting workflow list
  - `ActorRef<OrchestratorMsg>`: Reference to the orchestrator actor
  - `ListWorkflowsResponse`: HTTP response containing list of workflows
  - `WorkflowInfo`: Domain type representing workflow summary { invocation_id, name, status, started_at }
- Location: wtf-api/src/handlers.rs
- Assumptions:
  - The OrchestratorMsg enum has a ListWorkflows variant
  - ActorRef<OrchestratorMsg> is available via Extension layer
  - Extension<ActorRef<OrchestratorMsg>> provides access to the master actor
  - The orchestrator maintains a list of running workflow instances

## Preconditions
- P1: The `master` ActorRef must be available in Extension (no channel failure)
- P2: The orchestrator must be able to respond within a reasonable timeout

## Postconditions
- Q1: On success: HTTP 200 OK with ListWorkflowsResponse { workflows: Vec<WorkflowInfo> } is returned
- Q2: On actor communication failure: HTTP 500 Internal Server Error is returned
- Q3: On success: Returned workflows list may be empty (valid state)

## Invariants
- I1: The handler does not modify any state - it is a read-only operation
- I2: The returned WorkflowInfo accurately reflects current state from orchestrator

## Error Taxonomy
- Error::ActorCommunicationFailed - when call to orchestrator fails → 500
- Error::OrchestratorTimeout - when orchestrator doesn't respond in time → 500

## Contract Signatures
```rust
pub async fn list_workflows(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
) -> Result<Json<ListWorkflowsResponse>, StatusCode>
```

## Type Encoding
| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| master available | Compile-time | Extension<ActorRef<...>> |
| timeout handling | Runtime-checked | call_t with Duration |

## Violation Examples
- VIOLATES P1: If Extension is missing → compile-time error (not possible in properly configured router)
- VIOLATES P2: `list_workflows(Extension(...))` when orchestrator is dead → `Err(500 InternalServerError)`

## Ownership Contracts
- No ownership transfer - read-only operation
- master ActorRef is borrowed for the duration of the call

## Non-goals
- Filtering workflows by status (future enhancement)
- Pagination (future enhancement)
