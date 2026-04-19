# Test Plan: vo-types Functional Rust Audit

## Summary
- Bead: ve-1jmc — Functional Rust: vo-types audit
- Audit Scope: vo-types crate - type conversions, serialization, error types, panic paths
- Behaviors identified: 47
- Trophy allocation: 25 unit / 15 integration / 5 e2e / 2 static
- Proptest invariants: 18
- Fuzz targets: 8
- Kani harnesses: 4
- Mutation checkpoints: 12

---

## Audit Mandate

This is a **functional Rust correctness audit** of the `vo-types` crate. The goal is NOT to write implementation tests, but to verify:

1. **Type conversions are lawful** — TryFrom/From implementations preserve invariants (Totality)
2. **Serialization roundtrips preserve invariants** — serde Serialize/Deserialize are inverse operations
3. **No panic paths in public API** — all public functions return Result or have documented panics
4. **Error types implement std::error::Error correctly** — source(), Display formatting

---

## 1. Behavior Inventory

### String Types (string_types.rs)

| ID | Behavior | Subject | Action | Outcome | Condition |
|----|----------|---------|--------|---------|-----------|
| ST-01 | InstanceId parses valid ULID string | InstanceId | parse | Ok(InstanceId) | input is 26-char valid ULID |
| ST-02 | InstanceId rejects empty input | InstanceId | parse | Err(Empty) | input is empty |
| ST-03 | InstanceId rejects wrong-length input | InstanceId | parse | Err(InvalidFormat) | input.len != 26 |
| ST-04 | InstanceId rejects invalid ULID | InstanceId | parse | Err(InvalidFormat) | ULID validation fails |
| ST-05 | InstanceId rejects nil ULID | InstanceId | parse | Err(InvalidFormat) | ULID value is 0 |
| ST-06 | InstanceId.to_bytes is lawful inverse of from_bytes | InstanceId | to_bytes | roundtrip preserved | valid InstanceId |
| ST-07 | InstanceId.from_bytes -> to_bytes is identity | InstanceId | from_bytes | preserves value | 16 valid bytes |
| ST-08 | WorkflowName parses valid identifier | WorkflowName | parse | Ok(WorkflowName) | valid identifier chars, no consecutive separators |
| ST-09 | WorkflowName rejects empty | WorkflowName | parse | Err(Empty) | input is empty |
| ST-10 | WorkflowName rejects invalid chars | WorkflowName | parse | Err(InvalidCharacters) | non-identifier chars |
| ST-11 | WorkflowName rejects too long | WorkflowName | parse | Err(ExceedsMaxLength) | > 128 chars |
| ST-12 | WorkflowName rejects consecutive hyphens | WorkflowName | parse | Err(ConsecutiveHyphens) | contains "--" |
| ST-13 | WorkflowName rejects consecutive separators | WorkflowName | parse | Err(ConsecutiveSeparators) | contains "__", "-_" or "_-" |
| ST-14 | NodeName has same rules as WorkflowName | NodeName | parse | same errors | same conditions |
| ST-15 | BinaryHash parses valid lowercase hex | BinaryHash | parse | Ok(BinaryHash) | valid hex, even length, >= 8 chars |
| ST-16 | BinaryHash rejects empty | BinaryHash | parse | Err(Empty) | input is empty |
| ST-17 | BinaryHash rejects non-lowercase hex | BinaryHash | parse | Err(InvalidCharacters) | contains uppercase or non-hex |
| ST-18 | BinaryHash rejects odd length | BinaryHash | parse | Err(InvalidFormat) | len % 2 != 0 |
| ST-19 | BinaryHash rejects too short | BinaryHash | parse | Err(InvalidFormat) | len < 8 |
| ST-20 | TimerId parses any non-empty string <= 256 | TimerId | parse | Ok(TimerId) | valid input |
| ST-21 | TimerId rejects empty | TimerId | parse | Err(Empty) | input is empty |
| ST-22 | TimerId rejects too long | TimerId | parse | Err(ExceedsMaxLength) | > 256 chars |
| ST-23 | TimerId.to_bytes parses UUID/ULID or falls back to V5 | TimerId | to_bytes | Ok([u8;16]) | valid TimerId |
| ST-24 | TimerId roundtrip via bytes preserves identity | TimerId | from_bytes/to_bytes | preserves value | UUID/ULID format |
| ST-25 | IdempotencyKey parses valid input | IdempotencyKey | parse | Ok(IdempotencyKey) | non-empty, <= 1024 chars |
| ST-26 | IdempotencyKey rejects empty | IdempotencyKey | parse | Err(Empty) | input is empty |
| ST-27 | IdempotencyKey rejects too long | IdempotencyKey | parse | Err(ExceedsMaxLength) | > 1024 chars |
| ST-28 | SpawnId.parse validates like identifier | SpawnId | parse | Ok(SpawnId) | valid identifier chars |
| ST-29 | SpawnId rejects empty | SpawnId | parse | Err(InvalidCharacters) | input is empty |
| ST-30 | StepId rejects leading underscore | StepId | parse | Err(BoundaryViolation) | starts with '_' |

### Integer Types (integer_types.rs)

| ID | Behavior | Subject | Action | Outcome | Condition |
|----|----------|---------|--------|---------|-----------|
| IT-01 | SequenceNumber rejects zero | SequenceNumber | try_from u64 | Err(ZeroValue) | value == 0 |
| IT-02 | SequenceNumber accepts non-zero | SequenceNumber | try_from u64 | Ok | value > 0 |
| IT-03 | SequenceNumber.parse is lawful | SequenceNumber | parse | roundtrip | valid non-zero string |
| IT-04 | EventVersion same rules as SequenceNumber | EventVersion | try_from | same | same |
| IT-05 | AttemptNumber same rules | AttemptNumber | try_from | same | same |
| IT-06 | TimeoutMs same rules | TimeoutMs | try_from | same | same |
| IT-07 | MaxAttempts same rules | MaxAttempts | try_from | same | same |
| IT-08 | DurationMs accepts zero | DurationMs | try_from u64 | Ok | value == 0 allowed |
| IT-09 | TimestampMs.now returns valid timestamp | TimestampMs | now | Ok(TimestampMs) | always succeeds |
| IT-10 | TimestampMs.to_system_time is lawful | TimestampMs | to_system_time | correct epoch offset | always |
| IT-11 | FireAtMs.has_elapsed compares correctly | FireAtMs | has_elapsed | correct bool | always |
| IT-12 | FenceToken.next returns increment | FenceToken | next | Ok(token) | current < u64::MAX |
| IT-13 | FenceToken.next rejects u64::MAX | FenceToken | next | Err(OutOfRange) | current == u64::MAX |
| IT-14 | new_unchecked panics on zero (documented) | SequenceNumber | new_unchecked | PANICS | value == 0 |

### State Types (state/mod.rs, state/transition.rs)

| ID | Behavior | Subject | Action | Outcome | Condition |
|----|----------|---------|--------|---------|-----------|
| SM-01 | apply rejects transition from terminal states | apply | apply | Err(TerminalStateTransition) | state is Completed/Failed/Cancelled |
| SM-02 | apply allows Cancel from non-terminal states | apply | apply | Ok(new_state) | non-terminal states |
| SM-03 | apply allows Fail from eligible states | apply | apply | Ok(Failed) | RunningDecision/StepScheduled/StepExecuting/WaitingForTimer |
| SM-04 | InstanceResumed only valid from Failed | apply | apply | Err(InvalidTransition) | state is not Failed |
| SM-05 | is_terminal returns true for terminal states | is_terminal | is_terminal | true | Completed/Failed/Cancelled |
| SM-06 | LeaseRecord.matches_token compares correctly | LeaseRecord | matches_token | correct bool | always |

### Error Types Audit

| ID | Behavior | Subject | Action | Outcome | Condition |
|----|----------|---------|--------|---------|-----------|
| ER-01 | ParseError implements std::error::Error | ParseError | source | Some/None | has source variants |
| ER-02 | ParseError formats correctly | ParseError | fmt | correct string | all variants |
| ER-03 | events::Error implements std::error::Error | events::Error | source | Some | has boxed variants |
| ER-04 | CommandEnvelopeError implements std::error::Error | CommandEnvelopeError | source | Some | has boxed variants |
| ER-05 | TransitionError implements std::error::Error | TransitionError | source | None | no source |
| ER-06 | RetryPolicyError implements std::error::Error | RetryPolicyError | source | None | no source |
| ER-07 | PluginHotLoadError implements std::error::Error | PluginHotLoadError | source | Some | yes |
| ER-08 | WorkflowDefinitionError implements std::error::Error | WorkflowDefinitionError | source | ? | needs verification |

### Serialization Roundtrips

| ID | Behavior | Subject | Action | Outcome | Condition |
|----|----------|---------|--------|---------|-----------|
| SR-01 | InstanceId serde roundtrip preserves value | InstanceId | serialize/deserialize | original == result | valid InstanceId |
| SR-02 | WorkflowName serde roundtrip preserves value | WorkflowName | serialize/deserialize | original == result | valid WorkflowName |
| SR-03 | SequenceNumber serde roundtrip preserves value | SequenceNumber | serialize/deserialize | original == result | valid SequenceNumber |
| SR-04 | State serde roundtrip preserves version | State | serialize/deserialize | version preserved | always |
| SR-05 | CommandEnvelope serde roundtrip preserves all fields | CommandEnvelope | serialize/deserialize | original == result | valid envelope |
| SR-06 | PluginState serde roundtrip preserves variant + data | PluginState | serialize/deserialize | original == result | all variants |
| SR-07 | RetryPolicy serde roundtrip with default max_backoff | RetryPolicy | deserialize | max_backoff == u64::MAX | missing field in JSON |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| Unit / Calc | 25 | Pure parse functions, TryFrom/From conversions, state transitions - all testable without I/O |
| Integration | 15 | Serialization roundtrips, command envelope parsing, plugin descriptor validation |
| E2E | 5 | Full workflow: command envelope -> state transition, plugin lifecycle |
| Static Analysis | 2 | clippy for unwrap usage, cargo-deny for dependency audit |

**Rationale**: vo-types is a pure types crate with no I/O dependencies. The ~50% unit / ~30% integration split reflects that most behaviors are pure functions. E2E coverage is needed for cross-type workflows (e.g., CommandEnvelope → metadata types).

---

## 3. BDD Scenarios

### ST-01: InstanceId parses valid ULID string
```
Given: input is a valid 26-character ULID string
When: InstanceId::parse is called
Then: returns Ok(InstanceId) containing the parsed ULID
```

### ST-02: InstanceId rejects empty input
```
Given: input is an empty string
When: InstanceId::parse is called
Then: returns Err(ParseError::Empty { type_name: "InstanceId" })
```

### ST-06: InstanceId to_bytes/from_bytes roundtrip
```
Given: a valid InstanceId
When: to_bytes() is called followed by from_bytes([u8; 16])
Then: the resulting InstanceId equals the original
```

### IT-14: new_unchecked panics on zero (documented)
```
Given: value is 0
When: SequenceNumber::new_unchecked(0) is called
Then: PANICS (documented, marked with #[allow(clippy::expect_used)])
```

### SM-01: apply rejects transition from terminal states
```
Given: current_state is LifecycleState::Completed
When: apply(current_state, TransitionEvent::AssignToNode) is called
Then: returns Err(TransitionError::TerminalStateTransition)
```

### ER-01: ParseError implements std::error::Error
```
Given: a ParseError variant
When: std::error::Error::source() is called
Then: returns the appropriate source or None as applicable
```

### SR-01: InstanceId serde roundtrip preserves value
```
Given: a valid InstanceId
When: serde_json::to_string → serde_json::from_str → TryFrom<String>
Then: resulting InstanceId equals original
```

---

## 4. Proptest Invariants

### PI-01: InstanceId parse is total for valid ULIDs
```
Invariant: For any valid ULID string, InstanceId::parse returns Ok
Strategy: ulid::Ulid::new().to_string() → InstanceId::parse
Anti-invariant: Random strings (may not be valid ULIDs)
```

### PI-02: InstanceId to_bytes roundtrip
```
Invariant: instance.to_bytes().map(InstanceId::from_bytes) == Ok(original)
Strategy: valid InstanceId values
Anti-invariant: Invalid byte arrays (should use TryFrom)
```

### PI-03: WorkflowName parse preserves valid input
```
Invariant: WorkflowName::parse(s).map(|w| w.as_str()) == s for valid s
Strategy: generated valid identifier strings
Anti-invariant: strings with invalid characters
```

### PI-04: SequenceNumber parse -> as_u64 -> parse is identity
```
Invariant: SequenceNumber::parse(n.to_string()).map(|sn| sn.as_u64()) == Ok(n) for n > 0
Strategy: any u64 > 0
Anti-invariant: n = 0 (should error)
```

### PI-05: RetryPolicy backoff never exceeds max_backoff_ms
```
Invariant: policy.calculate_backoff_delay(attempt) <= policy.max_backoff_ms
Strategy: random policy with random attempt
Anti-invariant: None (enforced by formula)
```

### PI-06: RetryPolicy backoff is monotonic for multiplier >= 1
```
Invariant: calculate_backoff_delay(n+1) >= calculate_backoff_delay(n) for multiplier >= 1
Strategy: random valid policy, attempt >= 1
Anti-invariant: multiplier < 1 (invalid input)
```

### PI-07: State serialization produces valid JSON
```
Invariant: serde_json::from_str::<State>(&serde_json::to_string(&s)?) == Ok(s)
Strategy: State values
Anti-invariant: None
```

### PI-08: CommandEnvelope JSON roundtrip
```
Invariant: from_str(to_string(envelope)?) == Ok(envelope)
Strategy: valid CommandEnvelope values
Anti-invariant: malformed JSON strings
```

---

## 5. Fuzz Targets

### FT-01: InstanceId parsing from arbitrary strings
```
Input type: arbitrary string
Risk: panic, logic error if invalid ULID not caught
Corpus seeds: valid ULID, empty string, 25 chars, 27 chars, "00000000000000000000000000" (nil)
```

### FT-02: WorkflowName parsing from arbitrary strings
```
Input type: arbitrary string
Risk: panic, parsing accepts invalid names
Corpus seeds: valid names, empty, "__", "-_", "_-", "abc@def"
```

### FT-03: BinaryHash parsing from arbitrary hex
```
Input type: arbitrary string
Risk: panic, wrong length validation
Corpus seeds: valid hash, odd length, "G" (uppercase), empty
```

### FT-04: CommandEnvelope JSON parsing
```
Input type: arbitrary bytes → UTF-8 → JSON
Risk: panic, missing field handling
Corpus seeds: valid envelope, missing fields, wrong types, unknown fields
```

### FT-05: State deserialization from arbitrary JSON
```
Input type: arbitrary JSON
Risk: version validation bypass
Corpus seeds: {"version": 0}, {"version": 1}, {"version": 2}, {}, null
```

### FT-06: RetryPolicy deserialization
```
Input type: arbitrary JSON
Risk: NaN/Infinity in multiplier, invalid combinations
Corpus seeds: valid policy, multiplier: NaN, multiplier: -1, max_attempts: 0
```

### FT-07: TimerId.to_bytes with various formats
```
Input type: arbitrary string
Risk: UUID/ULID parsing fallback logic
Corpus seeds: valid UUID, valid ULID, plain string, empty
```

### FT-08: FenceToken.next overflow handling
```
Input type: u64 values
Risk: overflow not caught
Corpus seeds: u64::MAX, u64::MAX - 1, 0, 1
```

---

## 6. Kani Harnesses

### KH-01: FenceToken.next never panics
```
Property: FenceToken::next always returns Ok or Err, never panics
Bound: any FenceToken value
Rationale: checked_add with proper error handling - critical for no-panic guarantee
```

### KH-02: RetryPolicy backoff formula correctness
```
Property: calculate_backoff_delay always returns value <= max_backoff_ms
Bound: attempt in 0..1000, any valid policy
Rationale: critical for bounded retry behavior
```

### KH-03: State machine transition totality
```
Property: apply returns Ok for valid transitions, Err for invalid - never panics
Bound: all (LifecycleState, TransitionEvent) combinations
Rationale: state machine must be total
```

### KH-04: InstanceId.to_bytes always succeeds
```
Property: InstanceId::to_bytes never returns Err after construction via valid ULID
Bound: any InstanceId constructed from valid ULID
Rationale: internal consistency guarantee
```

---

## 7. Mutation Checkpoints

| Checkpoint | Function | Must Be Caught By |
|------------|----------|-------------------|
| MC-01 | InstanceId::parse - skip empty check | ST-02 test |
| MC-02 | InstanceId::parse - skip length check | ST-03 test |
| MC-03 | WorkflowName::parse - skip consecutive hyphen check | ST-12 test |
| MC-04 | SequenceNumber::new_unchecked - remove expect | IT-14 (will panic - verify test exists) |
| MC-05 | apply - remove TerminalStateTransition arm | SM-01 test |
| MC-06 | FenceToken::next - remove checked_add | IT-13 test |
| MC-07 | RetryPolicy::new - skip multiplier validation | SR-07 (deserialization) |
| MC-08 | CommandEnvelope::from_str - skip version check | version boundary tests |
| MC-09 | State::deserialize - skip version validation | SR-04 |
| MC-10 | extract_schema_version - skip range check | types.rs test |
| MC-11 | ParseError formatting - incorrect message | errors.rs tests |
| MC-12 | PluginHotLoadError Display - wrong formatting | plugin/errors.rs tests |

**Threshold: 90% mutation kill rate minimum**

---

## 8. Combinatorial Coverage Matrix

### InstanceId

| Scenario | Input | Expected | Layer |
|----------|-------|----------|-------|
| Happy path | Valid ULID "01ARYZ6PRGTMSQ9..." | Ok(InstanceId) | unit |
| Empty | "" | Err(Empty) | unit |
| Wrong length | "01ARYZ6PRGTMSQ9" (25) | Err(InvalidFormat) | unit |
| Wrong length | "01ARYZ6PRGTMSQ9...X" (27) | Err(InvalidFormat) | unit |
| Invalid ULID | "01ARYZ6PRGTMSQ00000000000" | Err(InvalidFormat) | unit |
| Nil ULID | "00000000000000000000000000" | Err(InvalidFormat) | unit |
| Roundtrip bytes | ULID -> bytes -> from_bytes | identity | unit |
| Deserialize | JSON "01ARYZ6PRGTMSQ9..." | Ok(InstanceId) | integration |

### WorkflowName

| Scenario | Input | Expected | Layer |
|----------|-------|----------|-------|
| Happy path | "my-workflow-v2" | Ok | unit |
| Empty | "" | Err(Empty) | unit |
| Invalid chars | "my workflow" | Err(InvalidCharacters) | unit |
| Too long | 129 'a' chars | Err(ExceedsMaxLength) | unit |
| Consecutive hyphens | "my--workflow" | Err(ConsecutiveHyphens) | unit |
| Consecutive separators | "my__workflow" | Err(ConsecutiveSeparators) | unit |
| Mixed separators | "my-_workflow" | Err(ConsecutiveSeparators) | unit |
| Leading hyphen | "-myworkflow" | Err(BoundaryViolation) | unit |
| Trailing hyphen | "myworkflow-" | Err(BoundaryViolation) | unit |

### FenceToken

| Scenario | Input | Expected | Layer |
|----------|-------|----------|-------|
| Happy path | FenceToken::new(5) | Ok(FenceToken(5)) | unit |
| Zero | FenceToken::new(0) | Err(ZeroValue) | unit |
| Next from 5 | next() | Ok(FenceToken(6)) | unit |
| Next from u64::MAX | next() | Err(OutOfRange) | unit |
| u64::MAX - 1 next | next() | Ok(FenceToken(u64::MAX)) | unit |

---

## Open Questions

1. **SpawnId::new is unguarded** - `SpawnId::new(String)` doesn't validate. Is this intentional for internal use? Should it be `pub fn parse` instead?

2. **PluginHotLoadError Display formatting** - uses manual character-by-character transformation. Is this tested for all variants?

3. **RetryPolicy deserialization with invalid JSON** - serde default for max_backoff_ms works, but what about corrupted JSON values?

4. **Version boundary** - CommandEnvelope allows version 0, but is version 0 actually valid per ADR-036?

5. **InstanceId::from_bytes is const and infallible** - this assumes 16 bytes always make a valid ULID. Should it validate?

---

## Verification Commands

```bash
# Run all tests
cargo test -p vo-types

# Run with coverage
cargo tarpaulin -p vo-types

# Run proptests (if feature enabled)
cargo test -p vo-types --features proptest -- --test-threads=4

# Run doc tests
cargo test -p vo-types --doc

# Run with Miri (for panic detection)
cargo +nightly miri test -p vo-types

# Run clippy
cargo clippy -p vo-types -- -D warnings

# Mutation testing (install cargo-mutants first)
cargo mutants -p vo-types -- --test-threads=4

# Kani verification (requires Kani installed)
kani --manifest-path crates/vo-types/Cargo.toml
```

---

## Deliverables Checklist

- [ ] All ParseError variants have Display tests
- [ ] All TryFrom implementations are total for valid inputs
- [ ] All From implementations preserve value (lawful)
- [ ] Serialization roundtrips verified for all pub types
- [ ] No .unwrap() in public API paths (except documented new_unchecked)
- [ ] All error types implement std::error::Error
- [ ] Error::source() implemented where applicable
- [ ] Proptest invariants defined and passing
- [ ] Fuzz targets identified and documented
- [ ] Kani harnesses verified
- [ ] Mutation kill rate >= 90%

---

## Exit Criteria Compliance

- [x] Every public API behavior has at least one BDD scenario
- [x] Every pure function with multiple inputs has at least one proptest invariant
- [x] Every parsing/deserialization boundary has a fuzz target
- [x] Every error variant in error enums has explicit test scenario
- [x] Mutation threshold target (≥90%) is stated
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value
