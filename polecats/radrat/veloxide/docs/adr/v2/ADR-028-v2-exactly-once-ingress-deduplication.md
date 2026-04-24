# ADR 028 (v2): Exactly-Once Ingress Deduplication

## Status
Accepted

## Context
Webhook providers, human users, retries from upstream load balancers, and continuation callbacks all produce duplicate requests in practice. If the Engine accepts each duplicate as a new logical event, exactly-once delivery is impossible before execution even begins.

## Decision
We implement durable ingress deduplication for every external event surface that participates in exact workflows.

### 1. Covered Ingress Surfaces
The dedupe contract applies to:
1. Workflow start triggers.
2. External signal or continuation callbacks.
3. Human approval submissions and similar resumptions.

### 2. Stable Dedupe Key Requirement
An exact workflow ingress request MUST carry a stable dedupe key supplied by the caller or derived from a provider-native event ID.

If an ingress event targets an exact workflow and does not provide a dedupe key, the Engine rejects it with a structured error instead of silently downgrading the guarantee.

### 3. Atomic Admission
On first acceptance, the Engine atomically writes:
1. the hashed dedupe record in `dedupe`,
2. the `WorkflowStarted` or `SignalAccepted` event,
3. the updated `InstanceSummary`.

The admission event also records the stable `command_id`, `correlation_id`, and `causation_id` metadata defined in ADR-036.

If the same dedupe key is seen again, the Engine returns the existing instance or signal outcome instead of creating duplicate work.

### 4. Retention Window
Dedupe records are retained until:
1. the instance reaches a terminal state, and
2. the configured dedupe retention window expires.

After expiry, the Engine may treat a repeated key as new work. The exact-once admission contract therefore applies within the configured retention window, which must be surfaced to operators.

For signal and continuation flows targeting long-lived workflows, dedupe scope may be lineage-aware rather than epoch-only as defined by ADR-038 and ADR-042.

## Consequences
- **Positive:** Exactly-once now starts at the network edge instead of only inside the Engine.
- **Positive:** Duplicate starts, approvals, and callbacks become deterministic no-ops.
- **Negative:** Callers must provide stable identity.
- **Negative:** Dedupe retention is now an explicit correctness and storage trade-off.
