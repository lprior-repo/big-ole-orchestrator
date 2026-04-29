# ADR 034 (v2): Saga Compensation and Reversibility

## Status
Accepted

## Context
Exactly-once commit does not solve business rollback.

If a workflow charges a card, reserves inventory, and then later fails while creating shipping, the system may have executed each committed effect exactly once and still left the business process in an unacceptable partial state.

This is a saga problem, not a duplicate-delivery problem.

## Decision
We model compensation explicitly for Managed Effect nodes.

### 1. Compensation Policy
Each Managed Effect node declares one of three policies:
1. `None`
   - The effect is irreversible.
   - If downstream work fails, the workflow enters a terminal failure that requires operator intervention.

2. `Automatic`
   - The node defines a compensator that the Engine may invoke automatically when the workflow enters a configured compensation path.

3. `Manual`
   - The node defines a compensator, but execution requires explicit operator approval.

### 2. Compensation Is Its Own Managed Effect
A compensation action is modeled as its own Engine-managed effect with:
1. its own logical `effect_id`,
2. its own prepare/commit journal,
3. the same exact-once sink requirements as a forward effect.

The Engine does not treat compensation as an in-process callback or a best-effort cleanup hook.

### 3. Workflow Semantics
- Compensation is triggered by explicit workflow policy, not by every retry.
- Retries of a failing node do not automatically compensate prior committed effects.
- Compensation order follows reverse dependency order of committed managed effects unless the workflow explicitly overrides it.
- Compensation states are represented explicitly in the hierarchical lifecycle model (ADR-039).

### 4. Connector Capability
Not every connector is reversible. A connector qualifies for `Automatic` or `Manual` compensation only if it can express a meaningful compensating action with a durable identity and reconciliation path.

Examples:
- Stripe capture may support refund.
- Inventory reservation may support release.
- Sending an email usually does not support true compensation and therefore remains irreversible.

## Consequences
- **Positive:** The architecture now distinguishes clearly between exact-once delivery and business rollback.
- **Positive:** Managed effects can participate in honest saga-style workflows without pretending every action is reversible.
- **Positive:** The future UI can surface reversibility and compensation policy directly on nodes.
- **Negative:** Compensation adds another lifecycle and another set of failure modes to design and test.
- **Negative:** Some workflows will remain partially irreversible by design and must expose that fact to operators.
