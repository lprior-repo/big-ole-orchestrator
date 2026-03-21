bead_id: wtf-c0l
bead_title: "bead: Implement get_journal handler"
phase: "STATE 1"
updated_at: "2026-03-20T23:15:00Z"

## Context
- Feature: Implement get_journal handler per ADR-012
- Domain terms:
  - `get_journal`: Axum handler that retrieves workflow execution journal/events
  - `OrchestratorMsg::GetJournal`: Message sent to the orchestrator actor
  - `ActorRef<OrchestratorMsg>`: Reference to the orchestrator actor
  - `JournalResponse`: API response containing invocation_id and journal entries
  - `JournalEntry`: Individual journal event with seq, type, name, input, output, timestamp
  - `invocation_id`: Unique identifier for a workflow invocation
- Location: wtf-api/src/handlers.rs
- Assumptions:
  - OrchestratorMsg enum has a GetJournal variant with invocation_id parameter
  - ActorRef<OrchestratorMsg> is available via Extension layer
  - Journal entries are stored and returned in seq order
  - Empty journal (no entries) is a valid response

## Preconditions
- P1: `invocation_id` path parameter must be a non-whitespace non-empty string
- P2: The `master` ActorRef must be available in Extension (no channel failure)
- P3: The workflow with given invocation_id must exist (404 returned if not)

## Postconditions
- Q1: On success: HTTP 200 OK with JournalResponse containing all journal entries
- Q2: On workflow not found: HTTP 404 is returned
- Q3: On actor communication failure: HTTP 500 Internal Server Error is returned
- Q4: Journal entries are returned in seq ascending order

## Invariants
- I1: Response contains all journal entries for the invocation (no entries omitted)
- I2: Empty entries array is valid (workflow has no events yet)
- I3: Each entry has seq, type, and timestamp fields populated (required fields non-null)

## Error Taxonomy
- Error::WorkflowNotFound { invocation_id } - when workflow doesn't exist → 404
- Error::ActorCommunicationFailed - when call to orchestrator fails → 500
- Error::InvalidInvocationId - when invocation_id is malformed/empty/whitespace → 400

## Contract Signatures
```rust
pub async fn get_journal(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(invocation_id): Path<String>,
) -> Result<Json<JournalResponse>, StatusCode>
```

## Type Encoding
| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| invocation_id non-empty/non-whitespace | Runtime-checked | String is_empty() or trim().is_empty() check |
| master available | Compile-time | Extension<ActorRef<...>> |
| workflow exists | Runtime-checked | Result error variant |
| entries sorted by seq | Runtime-checked | sort_by_key on returned Vec |

## Violation Examples (REQUIRED)
- VIOLATES P1 (empty): `get_journal(Extension(...), Path(""))` → `Err(400 BadRequest)`
- VIOLATES P1 (whitespace): `get_journal(Extension(...), Path("   "))` → `Err(400 BadRequest)`
- VIOLATES P2: If Extension is missing → compile-time error (not possible)
- VIOLATES P3: `get_journal(Extension(...), Path("nonexistent-id"))` → `Err(404 NotFound)`
- VIOLATES Q4: If entries returned out of order from orchestrator → handler MUST sort before returning 200, else data integrity violation

## Validation Order
1. Validate invocation_id (P1) FIRST - before any actor communication
2. Extract ActorRef from Extension (P2)
3. Send OrchestratorMsg::GetJournal to actor
4. Handle response (Q1-Q4)

## Ownership Contracts
- Shared borrow: `Extension(master)` - read-only access to actor ref
- No ownership transfer: handler borrows invocation_id from Path
- No mutation: function is async and returns data only

## Non-goals
- Creating or modifying workflow state
- Filtering or paginating journal entries
- Caching journal responses
