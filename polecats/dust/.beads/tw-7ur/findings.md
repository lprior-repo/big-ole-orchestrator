# Bead tw-7ur Findings: Workflow Start API Endpoint

## Summary
Implemented `POST /api/v1/workflows/{workflow_type}/start` endpoint that calls the orchestrator engine via `OrchestratorMsg::StartWorkflow`.

## Files Changed

### vo-api/src/v3/workflow.rs
- Added `start_workflow_by_type` handler that:
  - Extracts `workflow_type` from path
  - Accepts `namespace` and `input` from JSON body
  - Calls `orchestrator.send(OrchestratorMsg::StartWorkflow(...))` 
  - Returns `InstanceId` on success

### vo-api/src/v3/workspace.rs  
- Added route: `POST /api/v1/workflows/{workflow_type}/start`

### Test fixes (pre-existing issues):
- vo-api/tests/v3_api_todos.rs: Added `workflow_binary_hash` field to `V3StartRequest` test structs

## Key Signatures

**OrchestratorMsg::StartWorkflow** (in vo-orchestrator/src/):
```rust
pub struct StartWorkflow {
    pub workflow_type: String,
    pub namespace: String,
    pub input: serde_json::Value,
}
```

**Handler response**: Returns `InstanceId` on success, `StatusCode::INTERNAL_SERVER_ERROR` on failure.

## Verification
- `cargo build -p vo-api` passes
- Pre-existing clippy error in vo-types (not caused by these changes)
