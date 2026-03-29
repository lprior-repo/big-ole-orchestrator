bead_id: vo-acb
bead_title: vo-types: define all semantic newtypes
phase: state-1
updated_at: 2026-03-27T04:34:22Z

# Contract Specification: vo-acb -- vo-types: define all semantic newtypes

## Context

- **Feature**: A `vo-types` crate containing 14 semantic newtypes that enforce domain invariants at the type boundary. Every newtype exposes a `parse()` smart constructor returning `Result<Self, ParseError>`. The inner representation is never directly accessible -- only through controlled accessor methods. No `Default` derive is permitted on any validated domain type. No raw primitives cross public API boundaries.
- **Domain terms**:
  - `vo-types`: New crate in `crates/vo-types/`. Not yet listed in workspace `Cargo.toml` members (implementation concern).
  - `InstanceId`: Unique workflow instance identifier. ULID-encoded 26-character string (Crockford Base32). Used as Fjall key component (ADR-020).
  - `WorkflowName`: Human-readable workflow definition name. Constrained to `[a-zA-Z0-9_-]` characters. Used in `dag.add_node("name", fn)` (ADR-010) and CLI/API display.
  - `NodeName`: Human-readable node name within a DAG. Same character constraints as `WorkflowName`. Used in `dag.add_node("name", fn)` and Fjall event queries.
  - `BinaryHash`: Cryptographic hash of a compiled workflow binary. Hex-encoded string (SHA-256, 64 chars). Used to identify binary versions for execution (ADR-003).
  - `SequenceNumber`: Monotonically increasing event log position. `NonZeroU64`. Used as Fjall key suffix in big-endian encoding (ADR-020).
  - `EventVersion`: Schema version of an event type. `NonZeroU64`. First version is 1. Used for event evolution and deserialization routing.
  - `AttemptNumber`: Retry attempt counter for subprocess execution. `NonZeroU64`. First attempt is 1. Used in `tokio::time::timeout` retry logic (ADR-003).
  - `TimerId`: Unique timer identifier for hibernated workflows. Non-empty opaque string. Used as Fjall timer partition key component (ADR-005).
  - `IdempotencyKey`: Deduplication key for webhook triggers and external signals. Non-empty opaque string. Prevents duplicate workflow starts.
  - `TimeoutMs`: Subprocess execution timeout in milliseconds. `NonZeroU64`. Zero is invalid (a zero-duration timeout kills immediately, which is never intentional). Used in `tokio::time::timeout` (ADR-003).
  - `DurationMs`: Generic duration in milliseconds. `u64`. Zero is valid (e.g., "retry immediately"). Used for backoff intervals, hibernation durations, and metrics.
  - `TimestampMs`: Unix epoch timestamp in milliseconds. `u64`. Used as Fjall timer partition key prefix in big-endian encoding (ADR-020). Must be parseable from string representation of u64.
  - `FireAtMs`: Absolute timestamp when a hibernated workflow should reanimate. `u64`. Used in `TimerScheduled { fire_at_timestamp }` event (ADR-005). Must be parseable from string representation of u64.
  - `MaxAttempts`: Maximum number of execution attempts before permanent failure. `NonZeroU64`. Minimum is 1 (at least one attempt must be allowed). Used in subprocess retry policy (ADR-003).
  - `ParseError`: Unified error type returned by all `parse()` smart constructors. Carries the `type_name` of the originating newtype for error reporting.
- **Assumptions**:
  - The `ulid` crate (v1, already in workspace dependencies) provides ULID parsing/validation.
  - The `thiserror` crate (v1, already in workspace dependencies) is used for `ParseError` derivation.
  - The `serde` crate (v1 with derive, already in workspace dependencies) provides `Serialize`/`Deserialize` for all newtypes.
  - Serde deserialization MUST route through `parse()` -- no raw string/integer deserialization that bypasses validation.
  - The workspace `Cargo.toml` will be updated to include `"crates/vo-types"` in the `members` array (implementation concern).
  - `vo-common` (listed as "Shared types (WorkflowEvent, InstanceId)" in CLAUDE.md) will depend on `vo-types` rather than duplicating these definitions.
  - Timestamps use `u64` (not `i64`) per ADR-020 big-endian encoding convention. Pre-epoch timestamps are out of scope.
  - BinaryHash uses lowercase hex encoding only (uppercase rejected at parse time).
  - `TimerId` and `IdempotencyKey` are opaque strings with no format constraints beyond non-emptiness. Their internal structure (e.g., UUID, ULID, hash) is a caller concern.
- **Open questions**:
  - None. All domain constraints are derivable from the bead specification and ADRs.

## Preconditions

### For all `parse()` smart constructors:
- P-1: Input is a valid UTF-8 `&str`.
- P-2: No external state (DB, network, filesystem) is required for parsing.
- P-3: Parsing is deterministic: the same input always produces the same `Ok` or `Err` result.
- P-4: Parsing is side-effect-free: no logging, no allocation beyond the result.

### For string-validated newtypes (`InstanceId`, `WorkflowName`, `NodeName`, `BinaryHash`, `TimerId`, `IdempotencyKey`):
- P-5: `parse()` accepts `&str`.
- P-6: Leading/trailing whitespace is NOT stripped -- the caller is responsible for trimming before parsing.

### For integer-validated newtypes (`SequenceNumber`, `EventVersion`, `AttemptNumber`, `TimeoutMs`, `DurationMs`, `TimestampMs`, `FireAtMs`, `MaxAttempts`):
- P-7: `parse()` accepts `&str` and interprets it as a decimal integer.
- P-8: Hex, octal, or binary prefixes (`0x`, `0o`, `0b`) are NOT supported.
- P-9: Leading zeros are accepted (e.g., `"007"` parses to `7`).
- P-10: Negative sign (`-`) is rejected for all integer newtypes (all use `u64` inner type).

## Postconditions

### For all `parse()` smart constructors:
- PO-1: On `Ok(value)`, the returned newtype holds a value satisfying all invariants for its type.
- PO-2: On `Err(error)`, the error contains the correct `type_name` matching the newtype that failed.
- PO-3: The returned `ParseError` is human-readable via `Display` and machine-readable via its variants.
- PO-4: `parse()` never panics. All validation failures return `Err`.

### For `Display` implementations:
- PO-5: `to_string()` on any newtype produces a string that, when passed back to `parse()`, yields `Ok` (round-trip property).
- PO-6: `Display` output for string newtypes is the validated inner string (identity).
- PO-7: `Display` output for integer newtypes is the decimal representation of the inner value (no padding, no prefix).

### For accessor methods:
- PO-8: `as_str()` on string newtypes returns `&str` borrowing from the newtype (zero-copy).
- PO-9: `as_u64()` on integer newtypes returns the inner `u64` by value.
- PO-10: Accessors do NOT expose `&mut` references to the inner value. The inner value is immutable after construction.

### For serde:
- PO-11: `Serialize` produces the same string as `Display`.
- PO-12: `Deserialize` calls `parse()` internally. If `parse()` returns `Err`, deserialization fails with a clear error message.

## Invariants

### Universal invariants (all 14 newtypes):
- I-1: **No Default** -- No newtype derives `Default`. Constructing a newtype always requires explicit validation.
- I-2: **Immutability** -- The inner value is never mutated after construction. No `&mut` accessor exists.
- I-3: **Round-trip** -- For any valid value `v`: `parse(&v.to_string()) == Ok(v)`.
- I-4: **No public inner field** -- The inner representation is a private field. External code accesses it only through `as_str()` or `as_u64()`.
- I-5: **Debug transparency** -- `Debug` output includes the type name and the inner value (e.g., `InstanceId("01H5...")`).
- I-6: **Hash/Eq consistency** -- `Hash` and `Eq` are derived. Two values are equal iff their inner values are equal.
- I-7: **Clone is shallow** -- `Clone` copies the inner value. For string types, this is a `String::clone` (heap allocation).

### InstanceId invariants:
- I-10: Inner value is a valid ULID (26 characters, Crockford Base32).
- I-11: Inner value is exactly 26 characters long.
- I-12: All characters are in the Crockford Base32 alphabet: `[0-9A-Za-z]` (case-insensitive, 'I'/'i'/'L'/'l' mapped to '1', 'O'/'o' mapped to '0').

### WorkflowName invariants:
- I-13: Inner value is non-empty.
- I-14: All characters match `[a-zA-Z0-9_-]`.
- I-15: Inner value length is at most 128 characters.
- I-16: Inner value does not start or end with a hyphen (`-`) or underscore (`_`).

### NodeName invariants:
- I-17: Inner value is non-empty.
- I-18: All characters match `[a-zA-Z0-9_-]`.
- I-19: Inner value length is at most 128 characters.
- I-20: Inner value does not start or end with a hyphen (`-`) or underscore (`_`).

### BinaryHash invariants:
- I-21: Inner value is non-empty.
- I-22: All characters match `[0-9a-f]` (lowercase hex only).
- I-23: Inner value length is even (hex pairs represent complete bytes).
- I-24: Inner value length is at least 8 characters (minimum meaningful hash fragment).

### SequenceNumber invariants:
- I-25: Inner value is a `NonZeroU64` -- zero is structurally unrepresentable.
- I-26: Minimum value is 1.
- I-27: Maximum value is `u64::MAX` (18446744073709551615).

### EventVersion invariants:
- I-28: Inner value is a `NonZeroU64`.
- I-29: Minimum value is 1 (no version 0).
- I-30: Maximum value is `u64::MAX`.

### AttemptNumber invariants:
- I-31: Inner value is a `NonZeroU64`.
- I-32: Minimum value is 1 (first attempt).
- I-33: Maximum value is `u64::MAX`.

### TimerId invariants:
- I-34: Inner value is non-empty.
- I-35: Inner value length is at most 256 characters.

### IdempotencyKey invariants:
- I-36: Inner value is non-empty.
- I-37: Inner value length is at most 1024 characters.

### TimeoutMs invariants:
- I-38: Inner value is a `NonZeroU64` -- zero is structurally unrepresentable.
- I-39: Minimum value is 1 millisecond.
- I-40: Maximum value is `u64::MAX`.

### DurationMs invariants:
- I-41: Inner value is `u64` -- zero IS valid (instant duration).
- I-42: Minimum value is 0.
- I-43: Maximum value is `u64::MAX`.

### TimestampMs invariants:
- I-44: Inner value is `u64` (unsigned, per ADR-020 big-endian convention).
- I-45: Minimum value is 0 (Unix epoch).
- I-46: Maximum value is `u64::MAX`.

### FireAtMs invariants:
- I-47: Inner value is `u64` (unsigned, per ADR-020 big-endian convention).
- I-48: Minimum value is 0.
- I-49: Maximum value is `u64::MAX`.
- I-50: `FireAtMs` is not validated against "current time" at parse time. Whether `fire_at` is in the past is a runtime concern, not a parse concern.

### MaxAttempts invariants:
- I-51: Inner value is a `NonZeroU64`.
- I-52: Minimum value is 1.
- I-53: Maximum value is `u64::MAX`.

## Error Taxonomy

All `parse()` methods return `Result<NewType, ParseError>`. The `ParseError` enum is the single, exhaustive error type for the crate.

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// Input string was empty where non-empty was required.
    #[error("{type_name}: value must not be empty")]
    Empty {
        type_name: &'static str,
    },

    /// Input contained characters outside the allowed set.
    #[error("{type_name}: invalid characters: {invalid_chars:?}")]
    InvalidCharacters {
        type_name: &'static str,
        invalid_chars: String,
    },

    /// Input did not match the required format (ULID, hex, etc.).
    #[error("{type_name}: invalid format: {reason}")]
    InvalidFormat {
        type_name: &'static str,
        reason: String,
    },

    /// Input exceeded the maximum allowed length.
    #[error("{type_name}: exceeds maximum length of {max} (got {actual})")]
    ExceedsMaxLength {
        type_name: &'static str,
        max: usize,
        actual: usize,
    },

    /// Input violates a boundary constraint (e.g., starts/ends with hyphen).
    #[error("{type_name}: {reason}")]
    BoundaryViolation {
        type_name: &'static str,
        reason: String,
    },

    /// Input string could not be parsed as a u64.
    #[error("{type_name}: not a valid unsigned integer: {input}")]
    NotAnInteger {
        type_name: &'static str,
        input: String,
    },

    /// Parsed integer value was zero where nonzero was required.
    #[error("{type_name}: value must not be zero")]
    ZeroValue {
        type_name: &'static str,
    },

    /// Parsed integer value was outside the allowed range.
    #[error("{type_name}: value {value} is out of range (must be {min}..={max})")]
    OutOfRange {
        type_name: &'static str,
        value: u64,
        min: u64,
        max: u64,
    },
}
```

### Error mapping per newtype:

| Newtype | `Empty` | `InvalidCharacters` | `InvalidFormat` | `ExceedsMaxLength` | `BoundaryViolation` | `NotAnInteger` | `ZeroValue` | `OutOfRange` |
|---|---|---|---|---|---|---|---|---|
| `InstanceId` | yes | -- | yes (not valid ULID) | -- | -- | -- | -- | -- |
| `WorkflowName` | yes | yes | -- | yes (128) | yes (leading/trailing `-`/`_`) | -- | -- | -- |
| `NodeName` | yes | yes | -- | yes (128) | yes (leading/trailing `-`/`_`) | -- | -- | -- |
| `BinaryHash` | yes | yes (non-hex) | yes (odd length) | -- | -- | -- | -- | -- |
| `SequenceNumber` | -- | -- | -- | -- | -- | yes | yes | -- |
| `EventVersion` | -- | -- | -- | -- | -- | yes | yes | -- |
| `AttemptNumber` | -- | -- | -- | -- | -- | yes | yes | -- |
| `TimerId` | yes | -- | -- | yes (256) | -- | -- | -- | -- |
| `IdempotencyKey` | yes | -- | -- | yes (1024) | -- | -- | -- | -- |
| `TimeoutMs` | -- | -- | -- | -- | -- | yes | yes | -- |
| `DurationMs` | -- | -- | -- | -- | -- | yes | -- | -- |
| `TimestampMs` | -- | -- | -- | -- | -- | yes | -- | -- |
| `FireAtMs` | -- | -- | -- | -- | -- | yes | -- | -- |
| `MaxAttempts` | -- | -- | -- | -- | -- | yes | yes | -- |

### Error variant selection priority (first match wins):
1. `Empty` -- check length == 0 first (cheapest check).
2. `NotAnInteger` -- for integer newtypes, attempt `u64::from_str` and catch parse failure.
3. `ZeroValue` -- for integer newtypes requiring nonzero, check after successful parse.
4. `InvalidCharacters` -- for string newtypes, check each character against allowed set.
5. `InvalidFormat` -- for `InstanceId` (ULID validation) and `BinaryHash` (odd-length hex).
6. `ExceedsMaxLength` -- for string newtypes with length caps.
7. `BoundaryViolation` -- for `WorkflowName`/`NodeName` leading/trailing character rules.
8. `OutOfRange` -- reserved for future range constraints (e.g., `MaxAttempts` capped at some practical limit).

## Contract Signatures

### Module structure

```rust
// crates/vo-types/src/lib.rs
mod errors;
mod types;

pub use errors::ParseError;
pub use types::{
    InstanceId, WorkflowName, NodeName, BinaryHash,
    SequenceNumber, EventVersion, AttemptNumber,
    TimerId, IdempotencyKey,
    TimeoutMs, DurationMs, TimestampMs, FireAtMs,
    MaxAttempts,
};
```

### ParseError

```rust
// crates/vo-types/src/errors.rs
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("{type_name}: value must not be empty")]
    Empty { type_name: &'static str },

    #[error("{type_name}: invalid characters: {invalid_chars:?}")]
    InvalidCharacters { type_name: &'static str, invalid_chars: String },

    #[error("{type_name}: invalid format: {reason}")]
    InvalidFormat { type_name: &'static str, reason: String },

    #[error("{type_name}: exceeds maximum length of {max} (got {actual})")]
    ExceedsMaxLength { type_name: &'static str, max: usize, actual: usize },

    #[error("{type_name}: {reason}")]
    BoundaryViolation { type_name: &'static str, reason: String },

    #[error("{type_name}: not a valid unsigned integer: {input}")]
    NotAnInteger { type_name: &'static str, input: String },

    #[error("{type_name}: value must not be zero")]
    ZeroValue { type_name: &'static str },

    #[error("{type_name}: value {value} is out of range (must be {min}..={max})")]
    OutOfRange { type_name: &'static str, value: u64, min: u64, max: u64 },
}
```

### InstanceId

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InstanceId(/* private */ String);

impl InstanceId {
    /// Parse a ULID string into an InstanceId.
    /// Validates: non-empty, exactly 26 chars, valid Crockford Base32.
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Zero-copy borrow of the inner ULID string.
    pub fn as_str(&self) -> &str;
}

impl std::fmt::Display for InstanceId { /* ... */ }
```

### WorkflowName

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorkflowName(/* private */ String);

impl WorkflowName {
    /// Parse a workflow name string.
    /// Validates: non-empty, [a-zA-Z0-9_-] only, max 128 chars, no leading/trailing hyphen or underscore.
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Zero-copy borrow of the inner name string.
    pub fn as_str(&self) -> &str;
}

impl std::fmt::Display for WorkflowName { /* ... */ }
```

### NodeName

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NodeName(/* private */ String);

impl NodeName {
    /// Parse a node name string.
    /// Validates: non-empty, [a-zA-Z0-9_-] only, max 128 chars, no leading/trailing hyphen or underscore.
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Zero-copy borrow of the inner name string.
    pub fn as_str(&self) -> &str;
}

impl std::fmt::Display for NodeName { /* ... */ }
```

### BinaryHash

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BinaryHash(/* private */ String);

impl BinaryHash {
    /// Parse a hex-encoded binary hash string.
    /// Validates: non-empty, [0-9a-f] only (lowercase), even length, min 8 chars.
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Zero-copy borrow of the inner hex string.
    pub fn as_str(&self) -> &str;
}

impl std::fmt::Display for BinaryHash { /* ... */ }
```

### SequenceNumber

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct SequenceNumber(/* private */ std::num::NonZeroU64);

impl SequenceNumber {
    /// Parse a decimal string into a SequenceNumber.
    /// Validates: valid u64, nonzero.
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Get the inner value as u64.
    pub fn as_u64(self) -> u64;

    /// Construct from a known-nonzero u64. Panics if zero.
    /// Intended for internal use where the caller guarantees nonzero.
    pub fn new_unchecked(value: u64) -> Self;
}

impl std::fmt::Display for SequenceNumber { /* ... */ }
impl std::num::NonZeroU64: From<SequenceNumber> for NonZeroU64 { /* ... */ }
```

### EventVersion

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct EventVersion(/* private */ std::num::NonZeroU64);

impl EventVersion {
    /// Parse a decimal string into an EventVersion.
    /// Validates: valid u64, nonzero (minimum 1).
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Get the inner value as u64.
    pub fn as_u64(self) -> u64;

    /// Construct from a known-nonzero u64. Panics if zero.
    pub fn new_unchecked(value: u64) -> Self;
}

impl std::fmt::Display for EventVersion { /* ... */ }
```

### AttemptNumber

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct AttemptNumber(/* private */ std::num::NonZeroU64);

impl AttemptNumber {
    /// Parse a decimal string into an AttemptNumber.
    /// Validates: valid u64, nonzero (minimum 1).
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Get the inner value as u64.
    pub fn as_u64(self) -> u64;

    /// Construct from a known-nonzero u64. Panics if zero.
    pub fn new_unchecked(value: u64) -> Self;
}

impl std::fmt::Display for AttemptNumber { /* ... */ }
```

### TimerId

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TimerId(/* private */ String);

impl TimerId {
    /// Parse a timer identifier string.
    /// Validates: non-empty, max 256 chars.
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Zero-copy borrow of the inner timer ID string.
    pub fn as_str(&self) -> &str;
}

impl std::fmt::Display for TimerId { /* ... */ }
```

### IdempotencyKey

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdempotencyKey(/* private */ String);

impl IdempotencyKey {
    /// Parse an idempotency key string.
    /// Validates: non-empty, max 1024 chars.
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Zero-copy borrow of the inner key string.
    pub fn as_str(&self) -> &str;
}

impl std::fmt::Display for IdempotencyKey { /* ... */ }
```

### TimeoutMs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct TimeoutMs(/* private */ std::num::NonZeroU64);

impl TimeoutMs {
    /// Parse a decimal string into a TimeoutMs.
    /// Validates: valid u64, nonzero (minimum 1ms).
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Get the inner value as u64.
    pub fn as_u64(self) -> u64;

    /// Convert to `std::time::Duration`.
    pub fn to_duration(self) -> std::time::Duration;

    /// Construct from a known-nonzero u64. Panics if zero.
    pub fn new_unchecked(value: u64) -> Self;
}

impl std::fmt::Display for TimeoutMs { /* ... */ }
```

### DurationMs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct DurationMs(/* private */ u64);

impl DurationMs {
    /// Parse a decimal string into a DurationMs.
    /// Validates: valid u64. Zero is allowed.
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Get the inner value as u64.
    pub fn as_u64(self) -> u64;

    /// Convert to `std::time::Duration`.
    pub fn to_duration(self) -> std::time::Duration;
}

impl std::fmt::Display for DurationMs { /* ... */ }
```

### TimestampMs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct TimestampMs(/* private */ u64);

impl TimestampMs {
    /// Parse a decimal string into a TimestampMs.
    /// Validates: valid u64. Zero is allowed (Unix epoch).
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Get the inner value as u64.
    pub fn as_u64(self) -> u64;

    /// Convert to `std::time::SystemTime` (epoch-relative).
    pub fn to_system_time(self) -> std::time::SystemTime;

    /// Construct from current system time.
    pub fn now() -> Self;
}

impl std::fmt::Display for TimestampMs { /* ... */ }
```

### FireAtMs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct FireAtMs(/* private */ u64);

impl FireAtMs {
    /// Parse a decimal string into a FireAtMs.
    /// Validates: valid u64. Zero is allowed. No past-time check at parse time.
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Get the inner value as u64.
    pub fn as_u64(self) -> u64;

    /// Convert to `std::time::SystemTime` (epoch-relative).
    pub fn to_system_time(self) -> std::time::SystemTime;

    /// Check whether this fire-at time has elapsed relative to a given timestamp.
    pub fn has_elapsed(self, now: TimestampMs) -> bool;
}

impl std::fmt::Display for FireAtMs { /* ... */ }
```

### MaxAttempts

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct MaxAttempts(/* private */ std::num::NonZeroU64);

impl MaxAttempts {
    /// Parse a decimal string into a MaxAttempts.
    /// Validates: valid u64, nonzero (minimum 1).
    pub fn parse(input: &str) -> Result<Self, ParseError>;

    /// Get the inner value as u64.
    pub fn as_u64(self) -> u64;

    /// Check whether a given AttemptNumber has exhausted this limit.
    pub fn is_exhausted(self, attempt: AttemptNumber) -> bool;

    /// Construct from a known-nonzero u64. Panics if zero.
    pub fn new_unchecked(value: u64) -> Self;
}

impl std::fmt::Display for MaxAttempts { /* ... */ }
```

## Non-goals

- NG-1: No runtime validation of `FireAtMs` against "current time" -- that is the reanimator loop's responsibility (ADR-005).
- NG-2: No cryptographic validation of `BinaryHash` (e.g., verifying it is actually a SHA-256 output) -- format validation only.
- NG-3: No `Default` implementations on any newtype.
- NG-4: No `From<Primitive>` impls that bypass validation. Only `TryFrom` is acceptable.
- NG-5: No `new(value: primitive)` constructors that return the type directly. All construction goes through `parse()` or `try_from`.
- NG-6: No string interning or arena allocation. Each newtype owns its inner value.
- NG-7: No `PartialOrd`/`Ord` for string newtypes. Lexicographic ordering of names is not a domain concern.
- NG-8: No `Add`, `Sub`, or other arithmetic ops on newtypes. Domain arithmetic (e.g., incrementing `SequenceNumber`) is performed on the raw `u64` obtained via `as_u64()`.
- NG-9: No `FromStr` trait implementation. The `parse()` method is intentionally named to avoid confusion with the standard `FromStr` trait, whose error type (`ParseErrorKind`) does not carry `type_name` context.
- NG-10: No crate-level `Error` type wrapping `ParseError`. `ParseError` IS the error type.
- NG-11: No dependency on `chrono`, `time`, or any date/time library. Temporal semantics are handled at the caller level.
- NG-12: No `#[non_exhaustive]` on `ParseError` in this initial version. Variants are considered stable.
- NG-13: No `Copy` derive on string newtypes (`InstanceId`, `WorkflowName`, `NodeName`, `BinaryHash`, `TimerId`, `IdempotencyKey`). These are heap-allocated and must be `Clone` only.
- NG-14: No support for parsing `TimeoutMs`/`DurationMs`/`TimestampMs`/`FireAtMs` from human-friendly duration strings (e.g., `"30s"`, `"5m"`). Only decimal millisecond strings are accepted.
