# BLACKHAT Audit Findings: ve-y1g3

## Issue Summary
**Title:** 75+ raw String/u64 domain IDs despite existing NewTypes
**Severity:** CRITICAL (architectural debt, type safety violation)
**ADR References:** ADR-010 (Compile-Time DAG Type Safety), ADR-031 (Canonical WorkflowSpec)

## Executive Summary

Confirmed: The codebase suffers from **primitive obsession** - existing NewTypes in `vo-types` are not being used consistently, with 75+ instances of raw `String`/`u64` where type-safe alternatives exist.

## NewTypes Available (vo-types)

### String-based NewTypes (`string_types.rs`, `identity.rs`)
| Type | Purpose | Validation |
|------|---------|------------|
| `InstanceId` | Workflow instance ID | ULID format (26 chars, non-zero) |
| `StepId` | Step identifier | identifier chars, no leading underscore |
| `WorkflowName` | Workflow name | identifier chars, max 128 chars |
| `NodeName` | Node identifier | identifier chars, max 128 chars |
| `BinaryHash` | Binary hash | lowercase hex, even length |
| `TimerId` | Timer identifier | max 256 chars |
| `IdempotencyKey` | Idempotency key | identifier chars, max 1024 |
| `SpawnId` | Spawn identifier | identifier chars |
| `CommandId` | UUID-based command ID | UUID format |
| `CorrelationId` | UUID-based correlation ID | UUID format |
| `CausationId` | UUID-based causation ID | UUID format |

### Integer-based NewTypes (`integer_types.rs`)
| Type | Purpose | Validation |
|------|---------|------------|
| `TimestampMs` | Millisecond timestamp | u64 |
| `DurationMs` | Millisecond duration | u64 |
| `FireAtMs` | Fire-at timestamp | u64 |
| `TimeoutMs` | Timeout (non-zero) | NonZeroU64 |
| `FenceToken` | Fence token | NonZeroU64 |
| `AttemptNumber` | Attempt number | NonZeroU64 |
| `MaxAttempts` | Max attempts | NonZeroU64 |

## Findings: Raw Type Usage Counts

### `instance_id: String` — 34 instances
Key violations:
- `vo-storage/src/dedupe_partition/mod.rs:52,64,117,131` — `DedupeEntry.instance_id` and `DedupeRetentionRecord.instance_id` are `String` instead of `InstanceId`
- `vo-types/src/events/envelope.rs:10` — `EventEnvelope.instance_id` is `String`
- `vo-storage/src/event_store.rs:24` — raw `String`
- `vo-api/src/types/ingress.rs:36,41,75` — raw `String`
- `vo-api/src/types/v3.rs:29,37,83,105,130,137` — 6 instances

### `step_id: String` — 8 instances
Key violations:
- `vo-types/src/events/payload.rs:32,39,44,55,62,70` — 6 `step_id` fields in various event variants
- `vo-core/src/replay/types.rs:64` — raw `String`

### `workflow_name: String` — 13 instances
Key violations:
- `vo-types/src/command_history.rs:362,380` — 2 instances
- `vo-sdk/src/dag.rs:344` — raw `String`
- `vo-api/src/handlers/workflow.rs:30` — raw `String`
- `vo-api/src/handlers/workflow_lifecycle.rs:25` — raw `String`
- `vo-api/src/types/v1.rs:35,57` — 2 instances
- `vo-core/src/circuit_breaker/types.rs:62,66,74,79` — 4 instances in error variants

### `fire_at_ms: u64` — 42 instances
Key violations:
- `vo-types/src/events/payload.rs:78` — `TimerSet.fire_at_ms` should be `FireAtMs`
- `vo-storage/src/timer_index.rs:16,93,107,165,239` — multiple instances
- `vo-actor/src/timers.rs:80,88` — `TimerRecord.fire_at_ms` should be `FireAtMs`
- `vo-actor/src/timer_supervisor/types.rs:25,37` — should be `FireAtMs`

### `duration_ms: u64` — 20 instances
Key violations:
- `vo-storage/src/timer_index.rs:73,95,109,167` — should be `DurationMs`
- `vo-core/src/replay/event_sourcing_engine.rs:124,158` — should be `DurationMs`
- `vo-actor/src/timers.rs:29,134` — error variants and function params
- `vo-actor/src/timer_supervisor/types.rs:29,40` — should be `DurationMs`

## Architectural Violations

### Worst Files (by count)
1. `vo-types/src/events/payload.rs` — 24 raw fields
2. `vo-storage/src/timer_index.rs` — 10 raw fields
3. `vo-storage/src/dedupe_partition/mod.rs` — 7 raw fields
4. `vo-api/src/types/v3.rs` — 6 raw `instance_id` fields

### Mixed Usage Pattern (Anti-pattern)
Some files use both NewType and raw:
- `vo-actor/src/timer_lifecycle.rs:170` — takes `InstanceId` but passes `fire_at_ms: u64`
- `vo-storage/src/lease_partition/mod.rs` — correctly uses `InstanceId`, `StepId`, `FenceToken` in error types but `expires_at: u64` instead of `TimestampMs`

## Risk Assessment

### Type Confusion Attacks
Raw `String` for IDs enables:
- Accidental mixing of `instance_id` and `step_id` (both `String`)
- No compile-time prevention of semantic errors
- Silent coercion between different ID types

### Missing Validation
Raw `u64` for timestamps enables:
- Negative durations silently stored as large u64
- Overflow/underflow bugs in time comparisons
- No semantic distinction between `FireAtMs`, `TimestampMs`, `DurationMs`

### Serialization Inconsistency
`EventPayload::try_from_json` parses raw strings without validation against NewType parsers, bypassing type safety at the boundary.

## Recommendations

1. **Immediate:** Audit `vo-types/src/events/payload.rs` — this is the primary ingestion boundary and loses all type safety
2. **Short-term:** Replace `String` with `InstanceId` in `vo-storage/src/dedupe_partition/mod.rs`
3. **Medium-term:** Systematic replacement following ADR-010 enforcement
4. **Long-term:** Consider ADR-031 canonical type enforcement in CI (clippy lint)

## Verification Command
```bash
# Find all raw domain ID usages
grep -rn 'instance_id: String\|step_id: String\|workflow_name: String' crates/*/src/
grep -rn 'fire_at_ms: u64\|duration_ms: u64' crates/*/src/
```

## Status: Issue Confirmed
This is NOT a false positive. The NewTypes exist and are partially used, but the codebase has significant primitive obsession violations.