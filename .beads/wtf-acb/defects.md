# Defects Report: wtf-acb — wtf-types: define all semantic newtypes

**Reviewer**: Black Hat Reviewer
**Date**: 2026-03-27
**Verdict**: REJECTED

---

## PHASE 1: Contract & Bead Parity

### FAIL — WorkflowName/NodeName use `input.len()` not `input.chars().count()`

**Files**: `types.rs:203`, `types.rs:260`
**Severity**: MAJOR

The contract invariants I-15 and I-19 say "Inner value length is at most 128 **characters**." The contract defines character constraints as `[a-zA-Z0-9_-]` which are all ASCII, so `len()` and `chars().count()` are equivalent for *valid* inputs. However:

1. `ExceedsMaxLength { actual: input.len() }` reports **byte count** in the error, not character count. This is inconsistent with the contract terminology ("128 characters") and with TimerId/IdempotencyKey which correctly use `chars().count()` (lines 501, 550).
2. The `actual` field in `ExceedsMaxLength` is `usize` — fine. But it's reporting bytes, not chars, for WorkflowName/NodeName. A caller who reads "got 200" when they passed 50 two-byte characters would be confused.
3. This inconsistency is a **documentation lie**. Either both should use `chars().count()` or both should use `len()`. Pick one and be explicit.

**Fix**: Use `input.chars().count()` for WorkflowName and NodeName (matching TimerId/IdempotencyKey pattern), or explicitly document that these types are ASCII-only and `len()` == `chars().count()`.

### PASS — All 14 newtypes present

All 14 newtypes defined in the contract are implemented: `InstanceId`, `WorkflowName`, `NodeName`, `BinaryHash`, `SequenceNumber`, `EventVersion`, `AttemptNumber`, `TimerId`, `IdempotencyKey`, `TimeoutMs`, `DurationMs`, `TimestampMs`, `FireAtMs`, `MaxAttempts`.

### PASS — All parse() signatures match

Every `parse(&str) -> Result<Self, ParseError>` signature matches the contract.

### PASS — All accessor methods match

`as_str()` for string types, `as_u64()` for integer types, `to_duration()`, `to_system_time()`, `has_elapsed()`, `is_exhausted()`, `new_unchecked()`, `now()` — all present and correctly typed.

### PASS — ParseError enum matches contract exactly

All 8 variants present with correct field names and types. Error messages match the contract specification.

### PASS — Serde attributes correct

String types use `#[serde(try_from = "String", into = "String")]`. Integer types use `#[serde(try_from = "u64", into = "u64")]`. No validation bypass possible.

### PASS — Derives match contract

String types: `Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize` (no `Copy`, no `Default`). Integer types add `Copy, PartialOrd, Ord`. No `Default` on any type.

### PASS — Non-goals respected

- No `FromStr` impl (NG-9) ✓
- No `From<primitive>` bypass (NG-4) ✓
- No `Default` derive (NG-3) ✓
- No `PartialOrd`/`Ord` on string types (NG-7) ✓
- No arithmetic ops (NG-8) ✓
- No `#[non_exhaustive]` on `ParseError` (NG-12) ✓
- No `Copy` on string newtypes (NG-13) ✓

### PASS — Validation order matches contract priority

Empty → NotAnInteger → ZeroValue → InvalidCharacters → InvalidFormat → ExceedsMaxLength → BoundaryViolation. Verified in all parse() implementations.

---

## PHASE 2: Farley Engineering Rigor

### PASS — No function exceeds 25 lines

Longest production function: `check_identifier_boundaries` at 23 lines (39-61). All `parse()` methods are under 20 lines. `extract_invalid_chars` is 2 lines. Clean.

### PASS — No function has more than 5 parameters

All functions take 1-3 parameters. No violations.

### PASS — Pure logic separated from I/O

All `parse()` methods are pure functions. The sole impure method is `TimestampMs::now()` (line 693), which accesses system time. This is correctly documented in the implementation summary and is the minimal necessary impurity. No I/O hiding inside calculations.

### PASS — Tests assert behavior (WHAT), not implementation (HOW)

Tests assert exact error variants with exact field values. No testing of internal helper functions directly. No reliance on implementation details. Tests use the public `parse()` API exclusively.

### MINOR — Production `expect()` in `new_unchecked()` methods

**Files**: `types.rs:376, 423, 464, 608, 788`
**Severity**: MINOR

Five `expect()` calls exist in production code — all inside `new_unchecked()` methods. The contract explicitly says "Panics if zero" for these methods (lines 401, 427, 448, 510, 607), so this is **contractually correct**. However, `new_unchecked` is a footgun. The `#[doc(hidden)]` attribute or `unsafe` would better communicate the danger. This is noted but not blocking.

### MINOR — `mut` in test code only

**File**: `types.rs:3359, 3362`
Only two `mut` bindings, both in test code for `DefaultHasher`. Zero `mut` in production code. Clean.

---

## PHASE 3: NASA-Level Functional Rust (The Big 6)

### PASS — Illegal states unrepresentable

`NonZeroU64` makes zero structurally unrepresentable for `SequenceNumber`, `EventVersion`, `AttemptNumber`, `TimeoutMs`, `MaxAttempts`. `ParseError` enum exhaustively covers all validation failure modes. String newtypes require `parse()` — no direct construction possible from external code (field is `pub(crate)`).

### PASS — Parse, Don't Validate

All construction goes through `parse()` → `Result<Self, ParseError>`. Serde deserialization routes through `TryFrom` → `parse()`. No raw primitives cross public API boundaries. The boundary is enforced at the exact entry point.

### PASS — Types as Documentation

No boolean parameters anywhere. Method names are self-documenting. `has_elapsed`, `is_exhausted` return `bool` but take domain-typed parameters. `new_unchecked` is the only questionable name but is documented.

### PASS — Workflows are explicit state-to-state transitions

Not applicable — this is a pure data layer (Calc). No workflows or state machines exist in this crate. Correct.

### PASS — Newtypes wrap all primitives

Every domain concept is a newtype. `String` for text, `NonZeroU64` for nonzero integers, `u64` for zero-allowed integers. No raw primitives in public API signatures. All access through `as_str()`/`as_u64()`.

### PASS — No Option-based state machines

Not applicable — no state machines in this crate.

---

## PHASE 4: Ruthless Simplicity & DDD (Scott Wlaschin)

### PASS — No Option-based state machines

### PASS — CUPID properties satisfied
- **Composable**: Newtypes compose freely. `has_elapsed(FireAtMs, TimestampMs)` takes two domain types.
- **Unix-philosophy**: Each type does one thing. Helper functions are tiny and focused.
- **Predictable**: Same input always produces same output (deterministic). `TimestampMs::now()` is the exception but is documented.
- **Idiomatic**: Standard Rust patterns. `TryFrom`, `From`, `Display`, `Serialize`/`Deserialize`.
- **Domain-based**: Type names match domain terms from the contract exactly.

### MINOR — `check_identifier_boundaries` could be simpler

**File**: `types.rs:39-62`
The match on `(first, last)` with 4 arms is correct but slightly verbose. A single pass checking `starts_with`/`ends_with` would be more direct. Not blocking — it works, it's clear, it's under 25 lines.

### PASS — No cleverness detected

Code is boring, obvious, and readable. Expression-based control flow. No macros in production code. No trait objects. No dynamic dispatch. No unnecessary generics.

---

## PHASE 5: The Bitter Truth (Velocity & Legibility)

### MINOR — Contract gap: `+42` accepted by `u64::from_str`

**Found by test**: `types.rs:3803-3813`
The test `rq_plus_prefix_accepted_by_u64_from_str` documents that `+42` is silently accepted. Contract P-10 says "Negative sign (`-`) is rejected" but says nothing about the positive sign. Rust's `u64::from_str("+42")` succeeds. This is a contract documentation gap, not an implementation bug. The test correctly flags it.

**Recommendation**: Add P-11 to contract: "Positive sign (`+`) is rejected" — and reject it in `parse_u64_str` by checking `input.starts_with('+')`.

### MINOR — `InstanceId` case-sensitivity creates equality surprise

**Found by test**: `types.rs:3684-3696`
Two InstanceIds that differ only in case are NOT equal (`assert_ne!`), even though ULID is case-insensitive per Crockford Base32. The `Display` implementation preserves original case. This means `"01h5..." != "01H5..."` per I-6 (equality based on inner string). This is technically correct per the contract (I-6 says "Two values are equal iff their inner values are equal") but is a domain-level footgun. Consider normalizing to uppercase in `parse()`.

### MINOR — TimerId/IdempotencyKey accept control characters (null bytes, newlines)

**Found by test**: `types.rs:3848-3884`
These opaque types accept null bytes and newlines. While contractually correct ("opaque string with no format constraints beyond non-emptiness"), this is a storage/serialization hazard. Null bytes in JSON strings, database keys, or log lines will cause problems downstream.

### PASS — No YAGNI violations

Every type, method, and error variant serves a documented contract requirement. No generic handlers, no abstract traits with one implementer, no "future-proofing" code.

### PASS — The Sniff Test

Code looks like it was written by a disciplined engineer who followed the contract to the letter. No cleverness, no over-engineering, no showmanship. It's boring. It's correct.

---

## Summary

| Phase | Result | Findings |
|-------|--------|----------|
| 1. Contract Parity | **FAIL** | 1 MAJOR (len vs chars().count inconsistency) |
| 2. Farley Rigor | **PASS** | 2 MINOR (expect in new_unchecked is contractual, mut in tests only) |
| 3. Functional Rust (Big 6) | **PASS** | 0 findings |
| 4. Strict DDD | **PASS** | 1 MINOR (slightly verbose boundary check) |
| 5. Bitter Truth | **PASS** | 3 MINOR (contract gaps, not implementation bugs) |

### Tally
- **LETHAL**: 0
- **MAJOR**: 1
- **MINOR**: 6

### Aggregation Rule
0 LETHAL + 1 MAJOR (< 3) + 6 MINOR (≥ 5) = **REJECTED**

The 6 MINOR findings cross the ≥5 threshold. The 1 MAJOR (len vs chars().count inconsistency) must also be resolved.

---

## MANDATE

Before resubmission, the following must be resolved:

1. **[MAJOR]** Resolve the `len()` vs `chars().count()` inconsistency for `WorkflowName` and `NodeName` (`types.rs:203, 207, 260, 264`). Either:
   - Switch to `input.chars().count()` to match TimerId/IdempotencyKey pattern, OR
   - Document explicitly that these types are ASCII-only and `len()` is intentionally byte-based.

2. **[MINOR]** Reduce MINOR count below 5. Recommended resolution order:
   - Close the `+42` contract gap (add rejection or document acceptance)
   - Decide on InstanceId case normalization
   - Consider restricting control characters in TimerId/IdempotencyKey (or document the hazard)

Resubmit for full re-review from Phase 1 after changes.
