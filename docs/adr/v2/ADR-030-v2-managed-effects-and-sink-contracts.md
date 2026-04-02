# ADR 030 (v2): Managed Effects and Sink Contracts

## Status
Accepted

## Context
Exactly-once external side effects are impossible if an arbitrary subprocess can directly mutate Stripe, SMTP, SQL, or generic HTTP APIs and then crash before the Engine durably records the outcome.

## Decision
Exact-once external side effects must flow through **Managed Effect** nodes committed by the Engine.

### 1. One Logical Effect per Node
In v1 of the exact-once model, a Managed Effect node emits exactly one logical `EffectIntent`. If a workflow needs multiple side effects, it models them as multiple nodes.

### 2. Stable Effect Identity
Each Managed Effect has a stable `effect_id` scoped to the logical workflow step within the workflow lineage, not to the retry attempt. Retries advance the fence, not the logical effect identity. If a workflow explicitly needs epoch-local effect identity, that requirement must be declared in `WorkflowSpec` and justified by the connector semantics.

### 3. Connector Contract
An effect connector qualifies for exact-once only if it supports at least one of the following:
1. a native idempotency key,
2. a conditional write / compare-and-set primitive,
3. reconciliation by effect identity or a unique natural key.

The Engine persists `EffectPrepared` before invoking the connector and `EffectCommitted` after success. Recovery reconciles by `effect_id`.

The concrete connector runtime contract, timeout model, and reconcile/commit APIs are defined by ADR-041.

### 4. Unsupported Sinks
Blind side-effect sinks such as generic SMTP, arbitrary HTTP POST endpoints with no idempotency support, or opaque third-party APIs with no reconciliation capability do not qualify for exact-once.

They must be modeled as:
1. `Unsafe` nodes with at-least-once semantics, or
2. downstream relays/outboxes that provide their own dedupe semantics.

### 5. Compensation
Managed effects may optionally declare compensators, but compensation semantics are defined separately in ADR-034. Exact-once commit and compensation are related but distinct concerns.

## Consequences
- **Positive:** Exactly-once external effects become an honest, enforceable contract.
- **Positive:** Connector capability is explicit and testable instead of implicit hope.
- **Negative:** Some integrations are now clearly out of scope for exact-once unless wrapped by a stronger adapter.
- **Negative:** The Engine owns more connector logic than a naive subprocess-only design.
