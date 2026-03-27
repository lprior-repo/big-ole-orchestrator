# Architectural Drift Review — wtf-types

**Date**: 2026-03-27
**Crate**: `crates/wtf-types`
**Reviewer**: Automated (architectural-drift + scott-ddd-refactor skills)

## Summary

**STATUS: REFACTORED**

The original `types.rs` was **4252 lines** (813 production + 3439 tests), far exceeding the 300-line limit. The file has been split into focused submodules.

## Line Counts (Before → After)

| File | Before | After | Status |
|------|--------|-------|--------|
| `types.rs` | 4252 | 74 | PASS |
| `string_types.rs` | — | 246 | PASS |
| `integer_types.rs` | — | 208 | PASS |
| `errors.rs` | 133 | 133 | PASS |
| `lib.rs` | 9 | 17 | PASS |
| `serde_tests.rs` | — | 602 | N/A (test file) |
| `adversarial_tests.rs` | — | 627 | N/A (test file) |
| `cross_cutting_tests.rs` | — | 315 | N/A (test file) |

## Refactoring Actions

1. **Split `types.rs`** into 4 production files:
   - `types.rs` — shared helpers (`pub(crate)` parse/validation fns) + re-exports
   - `string_types.rs` — 6 string newtypes (InstanceId, WorkflowName, NodeName, BinaryHash, TimerId, IdempotencyKey)
   - `integer_types.rs` — 8 integer newtypes (SequenceNumber, EventVersion, AttemptNumber, TimeoutMs, MaxAttempts, DurationMs, TimestampMs, FireAtMs)
   - `errors.rs` — unchanged (already 133 lines)

2. **Extracted test modules** from inline `#[cfg(test)] mod tests` to separate files:
   - `serde_tests.rs` — serialize/deserialize/round-trip/corruption tests
   - `adversarial_tests.rs` — unicode edge cases, proptests, RED QUEEN invariants
   - `cross_cutting_tests.rs` — macro-driven cross-cutting integer edge cases

3. **DRY'd boilerplate** with macros to bring production files under 300:
   - `string_newtype!` — generates Display, TryFrom<String>, From<NewType> for String
   - `nonzero_newtype!` — generates Display, TryFrom<u64>, From<NewType> for NonZeroU64
   - `u64_newtype!` — generates Display, TryFrom<u64>, From<NewType> for u64

## Verification

- **Tests**: 230 passed, 0 failed
- **Clippy**: 0 warnings (with `-D warnings`)

## Scott Wlaschin DDD Assessment

### PASS — No Primitive Obsession
All 14 domain concepts are semantic newtypes. No raw `String`, `u64`, or `NonZeroU64` appear in public APIs.

### PASS — Parse, Don't Validate
Every newtype has a `::parse()` smart constructor that returns `Result<Self, ParseError>`. Core logic downstream accepts only trusted domain types.

### PASS — Explicit Error Taxonomy
`ParseError` enum has 8 typed variants (Empty, InvalidCharacters, InvalidFormat, ExceedsMaxLength, BoundaryViolation, NotAnInteger, ZeroValue, OutOfRange) — no opaque strings.

### PASS — Types as Spec
Function signatures enforce domain invariants at compile time:
- `NonZeroU64` inner types make zero values unrepresentable
- `pub(crate)` tuple fields prevent external construction without parsing
- Serde `try_from`/`into` ensures wire-format validation

### PASS — No Boolean Control Flags
No `bool` parameters in any domain API.

### PASS — No Option-as-State-Machine
No `Option` fields used to encode workflow state.

### FINDING — Minor: `+42` Accepted by Integer Parsers
`u64::from_str("+42")` silently succeeds. The contract (P-10) only forbids negative sign. This is a minor gap documented in existing tests (`rq_plus_prefix_accepted_by_u64_from_str`).

### FINDING — Minor: InstanceId Case Sensitivity
ULID crate accepts mixed-case but does NOT normalize. Two InstanceIds with different case are NOT equal per inner string comparison. This is documented in existing tests.

## File Structure

```
crates/wtf-types/src/
  lib.rs              (17 lines)  — module declarations + pub re-exports
  types.rs            (74 lines)  — shared helpers + re-exports
  string_types.rs    (246 lines)  — 6 string newtypes
  integer_types.rs   (208 lines)  — 8 integer newtypes
  errors.rs          (133 lines)  — ParseError enum
  serde_tests.rs     (602 lines)  — serde tests
  adversarial_tests.rs (627 lines) — adversarial + proptest
  cross_cutting_tests.rs (315 lines) — macro-driven edge cases
```
