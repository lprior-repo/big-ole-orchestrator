# ADR 040 (v2): Canonical Blob Durability and Publication

## Status
Accepted

## Context
Veloxide now distinguishes between hot control-plane records and cold canonical payload blobs.

That split improves throughput, but it creates a correctness hazard: if a control-plane event such as `StepCompleted` publishes an `output_ref` before the referenced canonical blob is actually durable, replay and forensics can observe a pointer to missing truth.

## Decision
Canonical payload blobs have an explicit publication protocol.

### 1. Blob Roles
1. **Routing-critical inline data**
   - small bounded data required for deterministic replay,
   - stored directly in the control plane as `routing_projection`.

2. **Canonical blobs**
   - full encrypted payloads or outputs,
   - stored in `payload_blobs`,
   - referenced by `output_ref` and `output_hash`.

### 2. Publication Rule
The Engine may only publish `output_ref` in a durable control-plane record after one of the following is true:
1. the blob was durably written before the batch that publishes the ref, or
2. the blob is staged and the same atomic storage primitive guarantees visibility of both the blob and the published ref together.

If neither is possible, the Engine must not publish the reference yet.

### 3. Failure Semantics
- If blob persistence fails before publication, the step stays incomplete and may be retried or failed according to policy.
- If the blob is optional for operator UX but not replay, the Engine may complete the step with only `routing_projection` and no `output_ref`.
- Replay must never require a blob that was only best-effort.

### 4. Product Discipline
The exact-once replay contract depends only on:
1. control-plane events,
2. routing projections,
3. effect journals,
4. workflow versions,
5. any canonical blobs that have crossed the publication boundary.

## Consequences
- **Positive:** Replay can never observe a durable pointer to a non-durable blob.
- **Positive:** Large outputs can stay off the hot path without corrupting exact-once semantics.
- **Negative:** Some step completions may be delayed until canonical blob durability is satisfied.
- **Negative:** Operator-visible full outputs become explicitly optional rather than automatically guaranteed.
