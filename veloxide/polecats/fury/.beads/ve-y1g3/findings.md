# BLACKHAT Audit Findings: ve-y1g3
## Primitive Obsession: 75+ Raw String/u64 Domain IDs Despite Existing NewTypes

**Date:** 2026-04-24
**Auditor:** polecat fury
**Severity:** P0 CRITICAL

---

## Executive Summary

The codebase has 75+ instances of raw `String` or `u64` domain IDs where NewType wrappers already exist. This is **primitive obsession** — a code smell that defeats compile-time type safety guarantees established by ADR-010 and ADR-031.

---

## Existing NewType Library

### String-based NewTypes (vo-types/src/string_types.rs, vo-types/src/dedupe.rs)

| NewType | File | Validation |
|---------|------|------------|
| `InstanceId` | string_types.rs:11 | ULID format, 26 chars, non-nil |
| `StepId` | string_types.rs:378 | identifier chars, no leading underscore |
| `NodeName` | string_types.rs:19 | identifier chars, max 128 |
| `WorkflowName` | string_types.rs:15 | identifier chars, max 128, no double hyphens |
| `BinaryHash` | string_types.rs:23 | lowercase hex, even length |
| `TimerId` | string_types.rs:27 | max 256 chars |
| `IdempotencyKey` | string_types.rs:31 | identifier chars, max 1024 |
| `SpawnId` | string_types.rs:35 | identifier chars |
| `DedupeKey` | dedupe.rs:15 | max 256 chars |

### Integer-based NewTypes (vo-types/src/integer_types.rs)

| NewType | Underlying | Notes |
|---------|------------|-------|
| `DurationMs` | u64 | No zero check |
| `TimestampMs` | u64 | No zero check |
| `FireAtMs` | u64 | No zero check |
| `FenceToken` | NonZeroU64 | Must be > 0 |
| `AttemptNumber` | NonZeroU64 | Must be > 0 |
| `SequenceNumber` | NonZeroU64 | Must be > 0 |
| `EventVersion` | NonZeroU64 | Must be > 0 |
| `TimeoutMs` | NonZeroU64 | Must be > 0 |
| `MaxAttempts` | NonZeroU64 | Must be > 0 |

---

## Detailed Findings by File

### 1. vo-types/src/events/payload.rs (24 raw fields)

**Severity: CRITICAL** — This is the canonical event wire format.

| Line | Field | Raw Type | Should Be |
|------|-------|----------|-----------|
| 12 | `workflow_id` | `String` | `InstanceId` |
| 14 | `binary_hash` | `String` | `BinaryHash` |
| 15 | `workflow_version_hash` | `String` | `BinaryHash` |
| 16 | `dedupe_key_hash` | `Option<String>` | `Option<BinaryHash>` |
| 19 | `workflow_id` | `String` | `InstanceId` |
| 20 | `completion_time_ms` | `u64` | `TimestampMs` |
| 23 | `workflow_id` | `String` | `InstanceId` |
| 24 | `failure_reason` | `String` | (freeform, OK) |
| 27 | `workflow_id` | `String` | `InstanceId` |
| 28 | `cancelled_by` | `String` | (freeform, OK) |
| 31 | `workflow_id` | `String` | `InstanceId` |
| 32 | `step_id` | `String` | `StepId` |
| 33 | `attempt` | `u32` | `AttemptNumber` |
| 34 | `fence` | `u64` | `FenceToken` |
| 35 | `execution_id` | `String` | `InstanceId` |
| 38 | `workflow_id` | `String` | `InstanceId` |
| 39 | `step_id` | `String` | `StepId` |
| 40 | `started_at_ms` | `u64` | `TimestampMs` |
| 43 | `workflow_id` | `String` | `InstanceId` |
| 44 | `step_id` | `String` | `StepId` |
| 45 | `completed_at_ms` | `u64` | `TimestampMs` |
| 46 | `attempt` | `u32` | `AttemptNumber` |
| 47 | `fence` | `u64` | `FenceToken` |
| 50 | `output_hash` | `Option<String>` | `Option<BinaryHash>` |

**Also in payload.rs:**
- Lines 54-58: `StepFailed` variant — same pattern as `StepScheduled`/`StepCompleted`
- Lines 61-66: `EffectPrepared` — `workflow_id`, `step_id`, `fence`
- Lines 69-73: `EffectCommitted` — `workflow_id`, `step_id`, `fence`
- Lines 76-78: `TimerSet` — `workflow_id`, `timer_id` (should be `TimerId`), `fire_at_ms` (should be `FireAtMs`)
- Lines 81-83: `TimerFired` — `workflow_id`, `timer_id`, `fired_at_ms` (should be `TimestampMs`)
- Lines 86-87: `CancelRequested` — `workflow_id`, `requested_by`
- Lines 90-91: `InstanceResumed` — `workflow_id`, `resumed_at_ms` (should be `TimestampMs`)
- Lines 95-98: `ContinuedAsNew` — `workflow_id`, `lineage_id`, `old_epoch`, `new_epoch`
- Lines 102-104: `WorkflowQuarantined` — `workflow_id`, `failure_window_seconds` (should be `DurationMs`)

**Count:** ~40 raw fields in payload.rs alone

---

### 2. vo-storage/src/lease_partition/mod.rs (10 raw fields)

**Severity: HIGH** — Lease storage is on the critical path.

| Line | Struct/Location | Field | Raw Type | Should Be |
|------|-----------------|-------|----------|-----------|
| 44 | `LeaseStoreError::LeaseAlreadyHeld` | `instance_id` | `String` | `InstanceId` |
| 45 | `LeaseStoreError::LeaseAlreadyHeld` | `step_id` | `String` | `StepId` |
| 49 | `LeaseStoreError::NotFound` | `instance_id` | `String` | `InstanceId` |
| 50 | `LeaseStoreError::NotFound` | `step_id` | `String` | `StepId` |
| 56 | `LeaseStoreError::FenceTokenExhausted` | `instance_id` | `String` | `InstanceId` |
| 57 | `LeaseStoreError::FenceTokenExhausted` | `step_id` | `String` | `StepId` |
| 74 | `LeaseEntry` | `instance_id` | `String` | `InstanceId` |
| 75 | `LeaseEntry` | `step_id` | `String` | `StepId` |
| 76 | `LeaseEntry` | `fence_token` | `u64` | `FenceToken` |
| 77 | `LeaseEntry` | `expires_at` | `u64` | `TimestampMs` |

**Note:** The `encode_lease_key`/`decode_lease_key` functions (lines 164-190) correctly use `InstanceId` and `StepId`.

---

### 3. vo-storage/src/dedupe_partition/mod.rs (10 raw fields)

**Severity: HIGH** — Dedupe is on the workflow start hot path.

| Line | Struct/Location | Field | Raw Type | Should Be |
|------|-----------------|-------|----------|-----------|
| 41 | `AdmissionResult::Duplicate` | `instance_id` | `String` | `InstanceId` |
| 51 | `DedupeEntry` | `dedupe_key` | `String` | `DedupeKey` |
| 52 | `DedupeEntry` | `instance_id` | `String` | `InstanceId` |
| 53 | `DedupeEntry` | `expires_at` | `u64` | `TimestampMs` |
| 116 | `DedupeRetentionRecord` | `dedupe_key` | `String` | `DedupeKey` |
| 117 | `DedupeRetentionRecord` | `instance_id` | `String` | `InstanceId` |
| 118 | `DedupeRetentionRecord` | `terminal_state_at` | `u64` | `TimestampMs` |
| 119 | `DedupeRetentionRecord` | `retention_expires_at` | `u64` | `TimestampMs` |

**Note:** The `encode_dedupe_key`/`decode_dedupe_key` functions (lines 207-224) correctly use `DedupeKey`.

---

### 4. vo-types/src/events/envelope.rs (3 raw fields)

| Line | Field | Raw Type | Should Be |
|------|-------|----------|-----------|
| 10 | `instance_id` | `String` | `InstanceId` |
| 12 | `timestamp_ms` | `u64` | `TimestampMs` |

---

### 5. vo-types/src/dual_representation.rs (4 raw fields)

| Line | Struct | Field | Raw Type | Should Be |
|------|--------|-------|----------|-----------|
| 94 | (anonymous) | `workflow_id` | `String` | `InstanceId` |
| 102 | (anonymous) | `workflow_id` | `String` | `InstanceId` |

---

### 6. vo-types/src/next_step_selection.rs (12 raw fields)

| Line | Struct | Field | Raw Type | Should Be |
|------|--------|-------|----------|-----------|
| 77 | StepDecision | `fence` | `u64` | `FenceToken` |
| 74 | StepDecision | `attempt` | `u32` | `AttemptNumber` |
| 93 | (anonymous) | `workflow_id` | `String` | `InstanceId` |
| 102 | (anonymous) | `fence` | `u64` | `FenceToken` |
| 99 | (anonymous) | `attempt` | `u32` | `AttemptNumber` |
| 112 | (anonymous) | `workflow_id` | `String` | `InstanceId` |
| 115 | (anonymous) | `fence` | `u64` | `FenceToken` |
| 114 | (anonymous) | `attempt` | `u32` | `AttemptNumber` |
| 280 | CandidateStep | `workflow_id` | `String` | `InstanceId` |

---

### 7. vo-types/src/recovery_contract.rs (1 raw field)

| Line | Struct | Field | Raw Type | Should Be |
|------|--------|-------|----------|-----------|
| 132 | (anonymous) | `minimum_fence` | `u64` | `FenceToken` |

---

### 8. vo-storage/src/receipts/mod.rs (2 raw fields)

| Line | Struct | Field | Raw Type | Should Be |
|------|--------|-------|----------|-----------|
| 34 | Receipt | `instance_id` | `String` | `InstanceId` |
| 48 | (constructor) | `instance_id` | `String` | `InstanceId` |

---

### 9. vo-storage/src/key_partition/mod.rs (2 raw fields)

| Line | Struct | Field | Raw Type | Should Be |
|------|--------|-------|----------|-----------|
| 113 | `DekNotFound` | `instance_id` | `String` | `InstanceId` |
| 117 | `DekAlreadyExists` | `instance_id` | `String` | `InstanceId` |

---

### 10. vo-storage/src/event_store.rs (1 raw field)

| Line | Struct | Field | Raw Type | Should Be |
|------|--------|-------|----------|-----------|
| 19 | (error variant) | `instance_id` | `String` | `InstanceId` |

---

## Architectural Violations

### ADR-010 (Compile-Time DAG Type Safety)

ADR-010 established that domain IDs should be wrapped in NewTypes to provide compile-time safety. The current state directly violates this:

1. **Type confusion**: A `u64` fence token passed to a function expecting a timestamp is not caught at compile time
2. **Invalid states representable**: Raw `String` can be empty or malformed; `InstanceId::parse()` validates ULID format
3. **No unit propagation**: `42u64` could be milliseconds, seconds, or an arbitrary token

### ADR-031 (Canonical WorkflowSpec)

ADR-031 defines canonical types for workflow identity. The raw `String` usage in payload.rs violates the semantic layering.

---

## Impact Assessment

| Category | Impact |
|----------|--------|
| **Runtime Bugs** | High — silent type confusion bugs in event processing |
| **Refactoring Risk** | High — grep for `String` won't find all workflow_id usages |
| **Performance** | Low — NewTypes are zero-cost wrappers |
| **Migration Effort** | Medium — requires coordinated change across crates |

---

## Recommendations

### Phase 1: Critical Path First (vo-types/events/payload.rs)

1. Replace all `workflow_id: String` with `workflow_id: InstanceId`
2. Replace all `step_id: String` with `step_id: StepId`
3. Replace all `fence: u64` with `fence: FenceToken`
4. Replace all `*_at_ms: u64` with `TimestampMs` or `FireAtMs`
5. Replace all `attempt: u32` with `AttemptNumber`

**Note:** This requires updating `try_from_json()` parsing logic to call `.parse()` on the raw strings.

### Phase 2: Storage Layer (vo-storage)

1. Update `LeaseEntry` to use `InstanceId`, `StepId`, `FenceToken`, `TimestampMs`
2. Update `DedupeEntry` to use `DedupeKey`, `InstanceId`, `TimestampMs`
3. Update error variants to use proper NewTypes

### Phase 3: Full Audit

Conduct a systematic grep for:
- `String` usages that should be domain IDs
- `u64` usages that should be temporal types
- `u32` usages for attempt counting

---

## Verification Command

```bash
# Find raw String domain IDs
grep -rn 'instance_id:\s*String\|step_id:\s*String\|workflow_id:\s*String' --include="*.rs" crates/vo-types/src/ crates/vo-storage/src/

# Find raw u64 temporal types
grep -rn '_ms:\s*u64\|_at:\s*u64\|duration_ms:\s*u64\|expires_at:\s*u64' --include="*.rs" crates/vo-types/src/ crates/vo-storage/src/
```

---

## Conclusion

The primitive obsession in this codebase is a **P0 architectural debt** item. The NewType library exists but is not being used consistently. Each raw `String` or `u64` is a potential runtime bug waiting to happen.

**Recommended Action:** Create a tracking epic for the migration and begin with Phase 1 (vo-types/events/payload.rs) as it is the canonical wire format affecting all workflows.
