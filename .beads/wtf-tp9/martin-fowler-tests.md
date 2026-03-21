bead_id: wtf-tp9
bead_title: "bead: Implement terminate_workflow handler"
phase: "STATE 1"
updated_at: "2026-03-20T22:37:00Z"

# Martin Fowler Test Plan

## Happy Path Tests
- test_terminate_workflow_returns_204_when_workflow_exists
- test_terminate_workflow_succeeds_for_valid_invocation_id

## Error Path Tests
- test_terminate_workflow_returns_404_when_workflow_not_found
- test_terminate_workflow_returns_400_when_invocation_id_empty
- test_terminate_workflow_returns_500_when_actor_communication_fails

## Edge Case Tests
- test_terminate_workflow_handles_special_characters_in_invocation_id
- test_terminate_workflow_idempotent_calls_return_same_result

## Contract Verification Tests
- test_precondition_invocation_id_not_empty
- test_precondition_master_actor_available
- test_postcondition_204_on_success
- test_postcondition_404_on_not_found
- test_invariant_no_partial_state_changes

## Given-When-Then Scenarios

### Scenario 1: Successfully terminate a running workflow
**Given**: A workflow with `invocation_id = "abc-123"` exists and is running
**When**: `terminate_workflow(Extension(master), Path("abc-123"))` is called
**Then**:
- Returns `Ok(204 No Content)`
- Workflow is stopped
- No response body is sent

### Scenario 2: Return 404 when workflow doesn't exist
**Given**: No workflow with `invocation_id = "nonexistent"` exists
**When**: `terminate_workflow(Extension(master), Path("nonexistent"))` is called
**Then**:
- Returns `Err(404 NotFound)`
- No modification to system state

### Scenario 3: Return 400 when invocation_id is empty
**Given**: An empty string is provided as invocation_id
**When**: `terminate_workflow(Extension(master), Path(""))` is called
**Then**:
- Returns `Err(400 BadRequest)`
- No attempt to contact actor

### Scenario 4: Handle actor communication failure
**Given**: The master ActorRef's channel is closed/failing
**When**: `terminate_workflow(Extension(master), Path("valid-id"))` is called
**Then**:
- Returns `Err(500 Internal Server Error)`
- Error is logged appropriately

## Specific Test Cases

### test_terminate_workflow_returns_204_when_workflow_exists
```rust
// Mock OrchestratorMsg::Terminate to succeed
// Call terminate_workflow with valid invocation_id
// Assert status is 204 No Content
```

### test_terminate_workflow_returns_404_when_workflow_not_found
```rust
// Mock OrchestratorMsg::Terminate to return WorkflowNotFound error
// Call terminate_workflow
// Assert status is 404 Not Found
```

### test_terminate_workflow_idempotent_calls_return_same_result
```rust
// First call returns 204
// Second call with same id also returns 204
// Verify no error about "already terminated"
```

## Exit Criteria
- [ ] All preconditions have corresponding runtime checks
- [ ] All postconditions are verifiable via status codes
- [ ] Error taxonomy maps to HTTP status codes correctly
- [ ] Test names describe behavior unambiguously
