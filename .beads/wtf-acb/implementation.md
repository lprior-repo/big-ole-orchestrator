# Implementation Summary: vo-acb -- vo-types: define all semantic newtypes

## Status: COMPLETE

**Result:** 310 tests passing (0 failures)

## Files Changed

| File | Change |
|------|--------|
| `crates/vo-types/Cargo.toml` | Added `ulid.workspace = true` dependency |
| `crates/vo-types/src/types.rs` | Replaced all stub implementations with full validation logic |
| `crates/vo-types/src/errors.rs` | Unchanged (was already correct) |
| `crates/vo-types/src/lib.rs` | Unchanged (was already correct) |

## Implementation Details

### Architecture: Pure Data Layer (Calc)

All 14 newtypes are implemented as **pure calculation functions** — no I/O, no mutation, no side effects. The `TimestampMs::now()` method is the sole impure boundary, accessing system time via `SystemTime::now()` with a safe fallback via `map_or(0, ...)`.

### Constraint Adherence

| Constraint | Status | Evidence |
|------------|--------|----------|
| **Zero mutability** | ✅ | Zero `mut` keywords in production code. All state flows through function parameters and return values. |
| **Zero panics/unwrap** | ✅ | No `unwrap()`, `expect()`, or `panic!()` in production code except `new_unchecked()` methods which are explicitly documented as "Panics if zero" per contract. |
| **No Default derive** | ✅ | None of the 14 newtypes derive `Default`. |
| **No From<Primitive>** | ✅ | No `From<u64>` or `From<String>` on validated types. Only `TryFrom` and `From<Newtype>` (outward conversions) are implemented. |
| **No FromStr** | ✅ | All constructors are `parse(&str) -> Result<Self, ParseError>` methods, not `FromStr` trait impls. |
| **No public inner fields** | ✅ | All inner fields are `pub(crate)` — accessible only within the crate (for tests), never from external code. |
| **NonZeroU64 for zero-invalid types** | ✅ | `SequenceNumber`, `EventVersion`, `AttemptNumber`, `TimeoutMs`, `MaxAttempts` all wrap `NonZeroU64`. |
| **BinaryHash constraints** | ✅ | Lowercase hex only (`[0-9a-f]`), even length, minimum 8 characters. |
| **WorkflowName/NodeName constraints** | ✅ | `[a-zA-Z0-9_-]` only, max 128 chars, no leading/trailing `-` or `_`. |
| **Expression-based** | ✅ | All `parse()` methods use early-return `?` operator and match expressions. |
| **Round-trip property** | ✅ | `Display` outputs identity for strings and decimal for integers. `parse(display(v)) == Ok(v)` holds for all valid values. |

### Validation Order (Per Contract Error Priority)

1. **Empty** — checked first (cheapest)
2. **NotAnInteger** — for integer types, `u64::from_str` failure
3. **ZeroValue** — for NonZeroU64 types after successful parse
4. **InvalidCharacters** — for string types, filter chars against allowed set
5. **InvalidFormat** — for InstanceId (ULID), BinaryHash (odd length / too short)
6. **ExceedsMaxLength** — for WorkflowName (128), NodeName (128), TimerId (256 chars), IdempotencyKey (1024 chars)
7. **BoundaryViolation** — for WorkflowName/NodeName (leading/trailing `-`/`_`)

### Helper Functions (Pure, Zero-Allocation Where Possible)

- `extract_invalid_chars(input, is_valid)` — filters chars not matching predicate
- `parse_u64_str(input, type_name)` — parses `&str` to `u64` with typed error
- `require_nonzero(value, type_name)` — wraps `u64` in `NonZeroU64` with typed error
- `parse_nonzero_u64(input, type_name)` — composes parse + nonzero check
- `is_identifier_char(c)` — `[a-zA-Z0-9_-]` predicate
- `is_lowercase_hex(c)` — `[0-9a-f]` predicate
- `check_identifier_boundaries(input, type_name)` — leading/trailing `-`/`_` check

### InstanceId Special Handling

The `ulid` crate v1 accepts all-zero ULIDs (`"00000000000000000000000000"`) as valid. Per the test specification, this is rejected with an explicit nil-value check (`ulid.0 == 0`) returning `InvalidFormat` with "invalid ULID validation" in the reason.

### TimerId/IdempotencyKey Character Counting

Max length validation uses `chars().count()` instead of `len()` to correctly handle multi-byte Unicode characters. The contract specifies "characters" (I-35, I-37), not bytes. This was verified by proptest which generates multi-byte Unicode strings.

### Serde Integration

All serde deserialization routes through `parse()` via `TryFrom`:
- String types: `#[serde(try_from = "String", into = "String")]`
- Integer types: `#[serde(try_from = "u64", into = "u64")]`

This ensures no validation bypass is possible through deserialization.

### Conversion Methods

- `TimeoutMs::to_duration()` → `Duration::from_millis(self.0.get())`
- `DurationMs::to_duration()` → `Duration::from_millis(self.0)`
- `TimestampMs::to_system_time()` → `UNIX_EPOCH + Duration::from_millis(self.0)`
- `TimestampMs::now()` → current system time in ms (safe fallback via `map_or`)
- `FireAtMs::to_system_time()` → `UNIX_EPOCH + Duration::from_millis(self.0)`
- `FireAtMs::has_elapsed(now)` → `self.0 < now.0`
- `MaxAttempts::is_exhausted(attempt)` → `attempt.as_u64() >= self.0.get()`
