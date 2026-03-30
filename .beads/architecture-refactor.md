# Architecture Refactor Report

## STATUS: REFACTORED

### Changes Made

#### 1. Removed Stale `vo-types/vo-types/` Duplicate Directory
- **What**: Deleted `/crates/vo-types/vo-types/` which was a stale copy of the vo-types source from a previous workspace restructuring.
- **Why**: Duplicate source trees cause confusion and drift. The real source is in `crates/vo-types/src/`.

#### 2. Extracted `payload_parser` Module from `events.rs`
- **File**: `crates/vo-types/src/events.rs` (1232 → 965 lines, source: ~563 → ~375 lines)
- **New File**: `crates/vo-types/src/payload_parser.rs` (157 lines including tests)
- **What**: Extracted repetitive JSON field extraction patterns into dedicated helper functions:
  - `require_string_field()` — for `workflow_id`/`step_id` style fields (InvalidPayloadField on missing)
  - `require_string()` — for `failure_reason`/`cancelled_by` style fields (MissingPayloadField on missing)
  - `require_u64()` — for required integer fields (e.g., `completion_time_ms`)
  - `optional_u64()` — for optional integer fields with defaults (e.g., `version`)
- **Why**: The original `try_from_json()` had 340 lines of near-identical field extraction code. Each event variant repeated the same 5-8 line pattern for every string/u64 field. The refactor collapses 12 variant handlers from ~330 lines to ~55 lines while preserving exact error semantics.
- **DDD Benefit**: "Parse, don't validate" — centralized parsing logic enforces consistent type coercion at the boundary. The extracted helpers serve as the single source of truth for how JSON fields map to domain types.

#### 3. Simplified `EventEnvelope::from_str` with Envelope Helpers
- **What**: Added `envelope_string()` and `envelope_u64()` private helpers for envelope field extraction (uses `Error::MissingEnvelopeField`/`Error::InvalidEnvelopeField`).
- **Why**: Same DRY principle — reduces repetitive `.get("field").ok_or_else(...)?.as_str().ok_or_else(...)?.to_string()` chains.

### Files Modified
| File | Before | After | Change |
|------|--------|-------|--------|
| `vo-types/src/events.rs` | 1232 lines | 965 lines | -267 lines |
| `vo-types/src/payload_parser.rs` | (new) | 157 lines | +157 lines |
| `vo-types/src/lib.rs` | 30 lines | 31 lines | +1 (module declaration) |
| `vo-types/vo-types/` | (stale dir) | (removed) | deleted |

### DDD Assessment (Scott Wlaschin)

**What's already good:**
- `string_types.rs`: Proper NewTypes (`InstanceId`, `WorkflowName`, `NodeName`, `BinaryHash`, `TimerId`, `IdempotencyKey`) with `parse()` constructors — "Parse, don't validate" ✓
- `integer_types.rs`: Proper NewTypes (`SequenceNumber`, `EventVersion`, `AttemptNumber`, `TimeoutMs`, etc.) using `NonZeroU64` where appropriate ✓
- `state.rs`: Exhaustive state machine with explicit transition rules — "Make illegal states unrepresentable" ✓
- `workflow/mod.rs`: Validated `WorkflowDefinition` with referential integrity and cycle detection ✓

**Remaining primitive obsession in `events.rs`:**
- `EventEnvelope.instance_id: String` → should be `InstanceId` (but InstanceId requires ULID format, envelope receives arbitrary strings from wire)
- `EventPayload.workflow_id: String` → should be a typed identifier
- `EventEnvelope.sequence: u64` → should be `SequenceNumber`
- `EventEnvelope.timestamp_ms: u64` → should be `TimestampMs`
- Event timestamp fields (`*_at_ms: u64`) → should be `TimestampMs`

These are serialization boundary types — changing them requires coordinated updates across `vo-storage`, `vo-api`, and `vo-worker`. Left for a follow-up refactor.

### Test Results
- All 855 vo-types lib tests pass (844 original + 11 new payload_parser tests)
- All workspace tests pass
- Full workspace `cargo check` succeeds
