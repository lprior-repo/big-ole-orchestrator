# ADR 036 (v2): Command Identity, Correlation, and Causation

## Status
Accepted

## Context
As the Engine grows exact-once admission, signals, retries, compensation, AI-driven edits, and operator actions, event logs become hard to reason about without durable lineage metadata.

The event-sourcing ecosystem has already solved this. We should steal the pattern directly.

## Decision
Every mutating API or CLI action enters the Engine as a versioned `CommandEnvelope`.

### 1. Command Metadata
Each command carries:
1. `command_id` - stable identity for dedupe and idempotent retries,
2. `correlation_id` - groups all work caused by a higher-level business request,
3. `causation_id` - points to the immediate parent event or command that caused this command,
4. `issuer` - system, API client, operator, AI agent, timer loop, recovery loop,
5. `issued_at` - physical timestamp for observability only.

### 2. Event Metadata
Every event emitted by the Engine records the command metadata that caused it plus the current workflow/version/fence context.

### 3. Idempotent Mutation Surfaces
The Engine uses `command_id` dedupe not only for workflow start, but also for:
1. external signals,
2. operator resume/cancel actions,
3. manual compensation requests,
4. quarantines and unquarantines,
5. any future mutating API.

## Consequences
- **Positive:** Exact-once semantics extend cleanly to operator and API command surfaces.
- **Positive:** UI, CLI, and AI tooling gain first-class traceability through business flows, retries, and compensations.
- **Positive:** Debugging becomes dramatically easier because every event has a durable reason.
- **Negative:** Events and command envelopes become more verbose.
