# Findings: tw-n4tq — vo-api: Forward dedupe_key to OrchestratorMsg::StartWorkflow

## Bug Summary
**ADR-028/043 Critical**: The dedupe_key is extracted and validated in `workflow_start.rs` but silently dropped. Exactly-once deduplication is non-functional.

## Root Cause
In `crates/vo-api/src/handlers/workflow_start.rs` lines 25-37:

```rust
let _dedupe_key = match req.dedupe_key {
    Some(ref key) if !key.is_empty() => key.clone(),
    _ => {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "missing_dedupe_key",
                "dedupe_key is required for exact workflow ingress (ADR-028)",
            )),
        )
            .into_response();
    }
};
```

The variable is named `_dedupe_key` (underscore prefix = intentionally unused). The key is validated for presence but **never forwarded** to `OrchestratorMsg::StartWorkflow`.

## Code Flow Analysis

### 1. workflow_start.rs:83-95 — Message Sent WITHOUT dedupe_key
```rust
let call_result = master
    .call(
        |tx| OrchestratorMsg::StartWorkflow {
            namespace,
            instance_id,
            workflow_type,
            paradigm,
            input,
            reply: tx,
        },
        Some(ACTOR_CALL_TIMEOUT),
    )
    .await;
```

### 2. vo-actor/src/lib.rs:65-72 — OrchestratorMsg::StartWorkflow Definition
```rust
StartWorkflow {
    namespace: NamespaceId,
    instance_id: InstanceId,
    workflow_type: String,
    paradigm: WorkflowParadigm,
    input: Bytes,
    reply: ractor::port::RpcReplyPort<Result<(), crate::StartError>>,
},
```
**Note**: No `dedupe_key` field exists in the message variant.

### 3. workflow.rs:142-154 — Same issue in v1 handler
The v1 `start_workflow_v1` handler in `workflow.rs` has the same problem — sends `OrchestratorMsg::StartWorkflow` without dedupe_key.

## Required Fix (3 files)

### File 1: `crates/vo-actor/src/lib.rs`
Add `dedupe_key: Option<String>` to `OrchestratorMsg::StartWorkflow`:
```rust
StartWorkflow {
    namespace: NamespaceId,
    instance_id: InstanceId,
    workflow_type: String,
    paradigm: WorkflowParadigm,
    input: Bytes,
    dedupe_key: Option<String>,   // <-- ADD THIS
    reply: ractor::port::RpcReplyPort<Result<(), crate::StartError>>,
},
```

### File 2: `crates/vo-api/src/handlers/workflow_start.rs`
Pass dedupe_key to the message (line 85):
```rust
let call_result = master
    .call(
        |tx| OrchestratorMsg::StartWorkflow {
            namespace,
            instance_id,
            workflow_type,
            paradigm,
            input,
            dedupe_key: Some(dedupe_key),  // <-- ADD THIS
            reply: tx,
        },
        Some(ACTOR_CALL_TIMEOUT),
    )
    .await;
```

### File 3: `crates/vo-api/src/handlers/workflow.rs`
Similar change needed in `start_workflow_v1` around line 144 — pass `req.dedupe_key` as `dedupe_key`.

## Additional Implementation Required
The orchestrator must implement:
1. Atomic check-and-insert for dedupe_key (using a dedupe cache/table)
2. Return `StartError::DuplicateDedupeKey` on conflict (or use existing `AlreadyExists`)
3. Return HTTP 409 CONFLICT when duplicate dedupe_key detected

## Error Response
Currently the code returns 400 BAD_REQUEST for missing dedupe_key. The dedupe_key is validated but not used. After fix, the dedupe_key validation should remain, and a new 409 CONFLICT response should be added for duplicates.

## Affected Endpoints
- `POST /api/v3/workflows` (V3StartRequest → workflow_start.rs)
- `POST /api/v1/workflows` (V1StartRequest → workflow.rs)

## Status
**This is an implementation task, not QA-only.** Code changes required in:
1. vo-actor/src/lib.rs — Add field to message enum
2. vo-api/src/handlers/workflow_start.rs — Forward dedupe_key
3. vo-api/src/handlers/workflow.rs — Forward dedupe_key
4. Orchestrator actor — Implement atomic dedupe check-and-insert
