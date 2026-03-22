bead_id: wtf-jht
bead_title: "bead: Implement list_workflows handler"
phase: "STATE 3"
updated_at: "2026-03-21T00:10:00Z"

## Implementation Summary

### Files Created
1. `crates/wtf-api/src/types.rs` - New file with API types including `ListWorkflowsResponse`
2. `crates/wtf-api/src/handlers.rs` - Added `list_workflows` handler

### Files Modified
1. `crates/wtf-api/src/lib.rs` - Added `types` module

### Implementation Details

#### list_workflows handler
- **Location**: `wtf-api/src/handlers.rs`
- **Signature**: `pub async fn list_workflows(Extension(master): Extension<ActorRef<OrchestratorMsg>>) -> Result<Json<ListWorkflowsResponse>, StatusCode>`
- **Pattern**: Follows existing `terminate_workflow` handler pattern

#### Contract Compliance
- ✅ P1: ActorRef availability via Extension (compile-time guaranteed)
- ✅ P2: Timeout handling with `call_t(..., Duration::from_secs(5))`
- ✅ Q1: Returns 200 OK with `ListWorkflowsResponse { workflows: Vec<WorkflowInfo> }`
- ✅ Q2: Returns 500 on actor communication failure
- ✅ Q3: Empty list is valid response
- ✅ I1: Read-only operation (no state modification)

#### Data->Calc->Actions Pattern
- **Data**: `ListWorkflowsResponse`, `WorkflowInfo` types from `wtf_actor::messages`
- **Calc**: `call_t` to orchestrator actor with timeout
- **Actions**: Return `Json(ListWorkflowsResponse)`

#### Zero Panics/Unwrap
- ✅ Uses `map_err` for error handling instead of `unwrap()`
- ✅ Returns `Result<T, StatusCode>` for explicit error propagation
- ✅ No `expect()`, `unwrap()`, or panic paths

### Tests Added
- `list_workflows_tests` module with:
  - Parametric tests for empty and non-empty lists
  - Contract verification tests for preconditions, postconditions, invariants

### Notes
- Implementation follows functional-rust principles (Data->Calc->Actions)
- Error handling uses `Result<T, E>` pattern
- Timeout of 5 seconds matches existing handler patterns

## Iteration 2 (contract hardening)

- Added explicit list handler test for empty-state read path:
  - `list_active_returns_empty_when_no_instances`

- Verification:
  - `cargo test -p wtf-actor master::handlers::list::tests -- --nocapture`
