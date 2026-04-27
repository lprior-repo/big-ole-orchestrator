# ADR 041 (v2): Managed Connector Runtime Contract

## Status
Accepted

## Context
ADR-030 defines which sinks qualify for exactly-once managed effects, but it does not define the actual runtime contract a connector must obey. Without that, every connector will invent its own semantics and exact-once will dissolve into adapter folklore.

## Decision
All managed connectors implement one uniform runtime contract.

### 1. Connector Operations
Each connector exposes the following logical operations:
1. `prepare(effect_intent, effect_id, fence) -> PreparedEffect`
2. `commit(prepared_effect) -> CommitOutcome`
3. `reconcile(effect_id) -> ReconcileOutcome`
4. `compensate(compensation_intent, compensation_effect_id, fence) -> CommitOutcome` when the connector supports compensation

### 2. Durability Sequence
The Engine sequence for a Managed Effect is:
1. child returns `EffectIntent`,
2. Engine validates and normalizes the intent,
3. Engine invokes `prepare(effect_intent, effect_id, fence)` to derive a normalized `PreparedEffect` value without committing the side effect,
4. Engine persists `EffectPrepared`,
5. Engine invokes `commit(prepared_effect)`,
6. on success, Engine persists `EffectCommitted` and `StepCompleted`,
7. on crash or ambiguity, Engine invokes `reconcile(effect_id)` during recovery.

### 3. Timeout and Ambiguity Model
- A connector timeout does not mean the effect failed.
- On timeout or transport ambiguity, the Engine records an ambiguous state and recovery must call `reconcile(effect_id)` before any retry.
- Retrying `commit` without reconciliation is forbidden unless the connector contract explicitly proves it is safe.

### 4. Receipts and Identity
- Every successful commit returns a durable receipt suitable for operator audit.
- Receipts must be persisted in `EffectCommitted`.
- Connector identity, version, and sink kind must be recorded so replay and forensics understand which adapter semantics were used.

### 5. Runtime Placement
In v1, connectors are in-process Engine components, not arbitrary child binaries.
If the system later externalizes connectors, they must preserve the exact same prepare/commit/reconcile semantics.

## Consequences
- **Positive:** Exactly-once managed effects now have a concrete runtime contract instead of hand-wavy connector promises.
- **Positive:** Recovery behavior under timeouts and ambiguous outcomes becomes deterministic.
- **Negative:** Connector implementation is more formal and more expensive than naive HTTP wrapper code.
