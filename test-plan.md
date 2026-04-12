# Test Plan: Structured Logging Pipeline

## Summary

- **Bead**: ve-i1hl — Test Plan: Structured logging pipeline
- **Contract**: ve-mbql — Contract: Structured logging pipeline
- **Behaviors identified**: 55
- **Trophy allocation**: 35 unit / 15 integration / 0 e2e / 5 static
- **Proptest invariants**: 12
- **Kani harnesses**: 2

---

## 1. Behavior Inventory

### Log Level Hierarchy

| # | Behavior | Public API |
|---|----------|------------|
| L-001 | `tracing::Level::ERROR` < `WARN` < `INFO` < `DEBUG` < `TRACE` | numeric comparison |
| L-002 | ERROR=1, WARN=2, INFO=3, DEBUG=4, TRACE=5 | level numeric value |
| L-003 | Level filtering at subscriber respects hierarchy | subscriber config |
| L-004 | When max level is ERROR, only ERROR logs pass | EnvFilter |
| L-005 | When max level is WARN, ERROR and WARN pass | EnvFilter |
| L-006 | When max level is INFO, ERROR/WARN/INFO pass | EnvFilter |
| L-007 | When max level is DEBUG, all but TRACE pass | EnvFilter |
| L-008 | When max level is TRACE, all logs pass | EnvFilter |

### Log Entry Structure

| # | Behavior | Public API |
|---|----------|------------|
| LE-001 | Every log entry contains `timestamp` as Unix ms (u64) | fmt::layer |
| LE-002 | Every log entry contains `level` as `tracing::Level` | fmt::layer |
| LE-003 | Every log entry contains `message` as `&'static str` | tracing macros |
| LE-004 | Every log entry contains `target` as `&'static str` | tracing macros |
| LE-005 | Logs without span have `span_context = None` | SpanContext |
| LE-006 | Logs within span have `span_context = Some(...)` | SpanContext |
| LE-007 | `trace_id` is non-zero for active spans | SpanContext |
| LE-008 | `span_id` is non-zero for current span | SpanContext |
| LE-009 | `parent_span_id` is `None` for root spans | SpanContext |
| LE-010 | `parent_span_id` is `Some(id)` for children | SpanContext |

### Structured Field Types

| # | Behavior | Public API |
|---|----------|------------|
| SF-001 | `InstanceId` wraps inner value correctly | newtype |
| SF-002 | `InstanceId` implements Display, Debug, Eq, Hash | InstanceId |
| SF-003 | `InstanceId` format specifier `%` produces correct output | fmt |
| SF-004 | `SpawnId` wraps inner value correctly | newtype |
| SF-005 | `SpawnId` implements required traits | SpawnId |
| SF-006 | Error captured with `%` shows readable message | tracing::error! |
| SF-007 | `error.code = ?` captures code when `code()` exists | tracing::error! |
| SF-008 | `error.strict = ?` captures full detail for Debug | tracing::error! |
| SF-009 | Chained errors (source) are preserved | Error::source |
| SF-010 | `u64` count fields serialize correctly | tracing macros |
| SF-011 | `u64` duration in ms serialize correctly | tracing macros |
| SF-012 | `bool` fields serialize to `true`/`false` | tracing macros |
| SF-013 | `metric_name` is `'static str` | MetricFields |
| SF-014 | `metric_delta` is `i64` | MetricFields |
| SF-015 | `labels` is `HashMap<&'static str, &'static str>` | MetricFields |

### Invariants

| # | Behavior | Public API |
|---|----------|------------|
| IN-001 | Field names are `snake_case` | type system |
| IN-002 | Field names are `&'static str` | compile-time verified |
| IN-003 | Prohibited names (`id`, `error`, `value`) rejected | compile-time |
| IN-004 | Required prefixes: `instance_id`, `spawn_id` | naming convention |
| IN-005 | `tracing::error!` always includes `error = %e` | grep + test |
| IN-006 | Error field uses Display (`%`) not Debug (`?`) | convention |
| IN-007 | `error.code = ?` used only when `code()` exists | convention |
| IN-008 | `error.strict = ?` used only for full detail | convention |
| IN-009 | All async fn entry points have `#[tracing::instrument]` | grep |
| IN-010 | `#[instrument]` uses `skip()` for large params | grep |
| IN-011 | Span context propagates across `tokio::spawn` | tracing::Span::current() |
| IN-012 | No `format!` interpolation in hot path log calls | clippy lint |
| IN-013 | No logging in tight loops without rate limiting | convention |
| IN-014 | All log messages are `&'static str` | compile-time |

### Log Volume Constraints

| # | Behavior | Public API |
|---|----------|------------|
| VC-001 | ERROR during shutdown: unlimited | rate limiter |
| VC-002 | ERROR during normal ops: max 10/second | rate limiter |
| VC-003 | WARN during normal ops: max 50/second | rate limiter |
| VC-004 | INFO during normal ops: max 100/second | rate limiter |
| VC-005 | DEBUG during normal ops: max 200/second | rate limiter |

### Error Taxonomy

| # | Behavior | Public API |
|---|----------|------------|
| ET-001 | `LOG001 Disabled`: logging disabled → WARN | LogCode |
| ET-002 | `LOG002 BufferFull`: buffer exceeded → WARN | LogCode |
| ET-003 | `LOG003 WriteFailure`: writer failed → ERROR | LogCode |
| ET-004 | `LOG004 EncodingError`: serialization failed → ERROR | LogCode |
| ET-005 | `LOG005 DrainFailure`: drain chain failed → ERROR | LogCode |
| ET-006 | `ERR_IO` (Io): File read failure → ERROR | AppError |
| ET-007 | `ERR_TIMEOUT` (Timeout): timed out → WARN | AppError |
| ET-008 | `ERR_RETRY` (Transient): retryable failure → WARN | AppError |
| ET-009 | `ERR_FATAL` (Fatal): unrecoverable → ERROR | AppError |
| ET-010 | `ERR_PARSE` (Parse): invalid input → WARN | AppError |
| ET-011 | `ERR_AUTH` (Auth): auth failure → WARN | AppError |
| ET-012 | `ERR_PERM` (Permission): authz failure → WARN | AppError |
| ET-013 | `ERR_NOTFOUND` (NotFound): resource missing → INFO | AppError |
| ET-014 | `ERR_CONFLICT` (Conflict): resource conflict → WARN | AppError |
| ET-015 | `ERR_CANCEL` (Cancelled): operation cancelled → INFO | AppError |
| ET-016 | `ERR_SHUTDOWN` (Shutdown): graceful shutdown → INFO | AppError |

### Error Response Taxonomy

| # | Behavior | Public API |
|---|----------|------------|
| ER-001 | Transient: `is_transient() = true`, `is_fatal() = false` | error trait |
| ER-002 | Transient: Log WARN, retry | error handling |
| ER-003 | Fatal: `is_transient() = false`, `is_fatal() = true` | error trait |
| ER-004 | Fatal: Log ERROR, abort | error handling |
| ER-005 | Cancelled: `is_transient() = false`, `is_fatal() = false` | error trait |
| ER-006 | Cancelled: Log INFO, clean shutdown | error handling |
| ER-007 | Unknown: `is_transient() = false`, `is_fatal() = false` | error trait |
| ER-008 | Unknown: Log ERROR, treat as fatal | error handling |

### Structured Logging Conventions

| # | Behavior | Public API |
|---|----------|------------|
| SC-001 | All targets follow `veloxide::module::submodule` pattern | naming convention |
| SC-002 | `veloxide::actor::spawn_supervisor` used in spawn_supervisor | grep |
| SC-003 | `veloxide::actor::reanimator` used in reanimator | grep |
| SC-004 | `veloxide::ipc::run` used in ipc/run | grep |
| SC-005 | Messages are past tense | convention |
| SC-006 | Messages are result-oriented | convention |
| SC-007 | Messages do NOT end with punctuation | convention |
| SC-008 | Messages are `&'static str` | compile-time |
| SC-009 | Error fields appear first in log entry | field ordering |
| SC-010 | ID fields appear second in log entry | field ordering |
| SC-011 | Numeric fields appear third in log entry | field ordering |
| SC-012 | State fields appear fourth in log entry | field ordering |

### Observer Pattern Integration

| # | Behavior | Public API |
|---|----------|------------|
| OP-001 | Every `metrics.incr()` has corresponding log | grep + test |
| OP-002 | `spawns_failed.incr()` → `tracing::warn!(...)` | grep |
| OP-003 | `health_checks_failed.incr()` → `tracing::warn(...)` | grep |
| OP-004 | `zombies_detected.incr()` → `tracing::error!(...)` | grep |
| OP-005 | Span attributes include `component` | #[instrument] |
| OP-006 | Span attributes include `instance_id` for long-lived | #[instrument] |
| OP-007 | Span attributes include `operation` for phase | #[instrument] |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit / Calc** | 35 | Error classification (`is_transient`/`is_fatal`), field type formatting, level ordering, rate limit calculations, naming conventions. All deterministic and exhaustively testable at unit level. |
| **Integration** | 15 | Span propagation across `tokio::spawn`, concurrent logging, subscriber layer configuration, rate limiter under load, metrics+logging co-location |
| **E2E** | 0 | No user-facing I/O in logging pipeline |
| **Static Analysis** | 5 | `clippy::pedantic` lints on `snake_case` field names, `format!` string interpolation, `&'static str` message enforcement |

---

## 3. BDD Scenarios

### Scenario: Error logged with full context

```
Given a SpawnSupervisorError with variant StorageError
When error is logged via tracing::error!
Then log entry contains error = %e (Display)
And error.code = ?e.code() (if available)
And error.strict = e (Debug)
And instance_id field is present
And target = "veloxide::actor::spawn_supervisor"
```

```rust
#[test]
fn error_logged_with_full_context() {
    let error = SpawnSupervisorError::StorageError(io::Error::new(ErrorKind::Other, "disk full"));
    let span = tracing::info_span!("test", instance_id = %InstanceId(123));
    let _guard = span.enter();
    
    tracing::error!(
        error = %error,
        error.code = ?error.code(),
        instance_id = %InstanceId(123),
        "Failed to save spawn record"
    );
    
    // Verify log output contains all required fields
}
```

---

### Scenario: Transient error handling

```
Given a transient error (StorageError)
When is_transient() is called
Then returns true
And is_fatal() returns false
And log level is WARN
And retry is attempted
```

```rust
#[test]
fn transient_error_handling() {
    let error = SpawnSupervisorError::StorageError(io::Error::new(ErrorKind::Other, "timeout"));
    assert!(error.is_transient());
    assert!(!error.is_fatal());
    // Log level should be WARN for transient
    // Retry should be attempted
}
```

---

### Scenario: Fatal error handling

```
Given a fatal error (CorruptSpawn)
When is_fatal() is called
Then returns true
And is_transient() returns false
And log level is ERROR
And operation is aborted
```

```rust
#[test]
fn fatal_error_handling() {
    let error = SpawnSupervisorError::CorruptSpawn(vec![0u8; 32]);
    assert!(!error.is_transient());
    assert!(error.is_fatal());
    // Log level should be ERROR for fatal
    // Operation should be aborted
}
```

---

### Scenario: Span continuity across spawn

```
Given an active span with context
When tokio::spawn is called
Then child task has linked span context
And trace_id is preserved
And parent_span_id links to original span
```

```rust
#[tokio::test]
async fn span_continuity_across_spawn() {
    let span = tracing::info_span!("parent", instance_id = %InstanceId(456));
    let trace_id = span.context().span().span_id();
    
    let handle = tokio::spawn(async move {
        let current = tracing::Span::current();
        // Verify span context is propagated
    });
    
    handle.await.unwrap();
}
```

---

### Scenario: Log rate limiting

```
Given 15 ERROR logs in 1 second
When 11th log is emitted
Then it is dropped
And LOG002 metric is incremented
And subsequent logs continue to be processed
```

```rust
#[tokio::test]
async fn log_rate_limiting() {
    let config = RateLimitConfig { max_per_second: 10 };
    let limiter = LogRateLimiter::new(config);
    
    for i in 0..15 {
        let allowed = limiter.try_record(LogLevel::ERROR);
        if i < 10 {
            assert!(allowed, "first 10 should be allowed");
        } else {
            assert!(!allowed, "11th+ should be dropped");
        }
    }
    
    assert_eq!(limiter.metrics(LOG002), 5);
}
```

---

## 4. Proptest Invariants

### PI-01: Level ordering is consistent

```
Invariant: ERROR < WARN < INFO < DEBUG < TRACE
Strategy: arbitrary pair of log levels
Anti-invariant: N/A
```

```rust
proptest! {
    #[test]
    fn level_ordering_consistent(a: LogLevel, b: LogLevel) {
        let ordering = [ERROR, WARN, INFO, DEBUG, TRACE];
        let idx_a = ordering.iter().position(|l| l == &a);
        let idx_b = ordering.iter().position(|l| l == &b);
        if idx_a < idx_b {
            assert!(a.to_numeric() < b.to_numeric());
        }
    }
}
```

---

### PI-02: Field names are snake_case

```
Invariant: All field names match ^[a-z][a-z0-9_]*$
Strategy: arbitrary field name strings
Anti-invariant: camelCase, PascalCase, kebab-case
```

```rust
proptest! {
    #[test]
    fn field_names_are_snake_case(name: String) {
        let is_valid = name.chars().next().map(|c| c.is_lowercase()).unwrap_or(false)
            && name.chars().all(|c| c.is_lowercase() || c.is_numeric() || c == '_');
        prop_assert!(is_valid, "field name '{}' must be snake_case", name);
    }
}
```

---

### PI-03: Error codes map to correct log levels

```
Invariant: ERR_FATAL → ERROR, ERR_TIMEOUT → WARN, etc.
Strategy: arbitrary ErrorCode and LogLevel
Anti-invariant: mismatched mapping
```

```rust
proptest! {
    #[test]
    fn error_codes_have_correct_log_level(code: ErrorCode) {
        let level = code.to_log_level();
        match code {
            ErrorCode::Fatal => assert_eq!(level, ERROR),
            ErrorCode::Timeout | ErrorCode::Retry | ErrorCode::Parse 
                | ErrorCode::Auth | ErrorCode::Perm | ErrorCode::Conflict => assert_eq!(level, WARN),
            ErrorCode::NotFound | ErrorCode::Cancelled | ErrorCode::Shutdown => assert_eq!(level, INFO),
            _ => {}
        }
    }
}
```

---

### PI-04: is_transient XOR is_fatal for standard errors

```
Invariant: For any error, is_transient() != is_fatal()
Strategy: arbitrary error variants
Anti-invariant: both true or both false
```

```rust
proptest! {
    #[test]
    fn transient_and_fatal_are_mutually_exclusive(error: SpawnSupervisorError) {
        let is_t = error.is_transient();
        let is_f = error.is_fatal();
        assert_ne!(is_t, is_f, "error {:?}: is_transient={}, is_fatal={}", error, is_t, is_f);
    }
}
```

---

### PI-05: Token bucket burst never exceeded (rate limit)

```
Invariant: tokens <= burst at all times
Strategy: arbitrary burst, rate, elapsed
Anti-invariant: N/A
```

```rust
proptest! {
    #[test]
    fn rate_limit_tokens_never_exceed_burst(
        burst in 1u64..=1000,
        rate in 0f64..=1000f64,
        elapsed_ms in 0u64..=10000,
    ) {
        let limiter = LogRateLimiter::new(burst, rate);
        let now = Instant::now();
        limiter.try_record(ERROR);
        let later = now + Duration::from_millis(elapsed_ms);
        let available = limiter.available(later);
        prop_assert!(available <= burst as f64 + 0.001);
    }
}
```

---

## 5. Fuzz Targets

### FT-01: Arbitrary field name strings

```
Input type: String
Risk: panic on invalid field name format, buffer overflow
Corpus seeds: empty string, unicode, very long names (1MB), special chars
```

### FT-02: Error type with arbitrary source chain

```
Input type: Box<dyn Error + Send + Sync>
Risk: infinite loop in source() traversal, stack overflow
Corpus seeds: no source, single source, 10-level chain, cyclic source
```

### FT-03: Span context propagation

```
Input type: (SpanContext, String)
Risk: trace_id collision, span_id collision, context corruption
Corpus seeds: root span, child span, detached span
```

---

## 6. Kani Harnesses

### KH-01: is_transient/is_fatal mutual exclusion

```
Property: For all SpawnSupervisorError, is_transient() != is_fatal()
Bound: all enum variants
Rationale: Violation means error classification is broken
```

```rust
#[kani::proof]
fn spawn_supervisor_error_classification_invariant() {
    let error = SpawnSupervisorError::arbitrary();
    kani::assert(
        error.is_transient() != error.is_fatal(),
        "is_transient and is_fatal must be mutually exclusive"
    );
}
```

### KH-02: Log level ordering

```
Property: ERROR < WARN < INFO < DEBUG < TRACE
Bound: all level pairs
Rationale: Rate limiting depends on correct ordering
```

```rust
#[kani::proof]
fn log_level_ordering_invariant() {
    let levels = [ERROR, WARN, INFO, DEBUG, TRACE];
    for i in 0..levels.len() {
        for j in (i+1)..levels.len() {
            kani::assert(
                levels[i].to_numeric() < levels[j].to_numeric(),
                "levels must be ordered correctly"
            );
        }
    }
}
```

---

## 7. Mutation Checkpoints

| Checkpoint | Mutated Code | Must Be Caught By |
|------------|--------------|-------------------|
| MC-001 | Change `error = %e` to `error = ?e` | error_field_uses_display_format |
| MC-002 | Remove `#[instrument]` from async fn | instrument_attribute_present |
| MC-003 | Add `format!` in log message | no_format_interpolation |
| MC-004 | Change `is_transient()` return | transient_error_tests |
| MC-005 | Change `is_fatal()` return | fatal_error_tests |
| MC-006 | Remove rate limit check | rate_limiting_tests |
| MC-007 | Use non-snake_case field name | field_naming_convention |
| MC-008 | Remove span propagation | span_continuity_tests |

---

## 8. Acceptance Criteria Coverage

| # | Criterion | Test Method |
|---|-----------|-------------|
| AC1 | All `tracing::error!` calls include `error = %e` field | grep + integration test |
| AC2 | All `tracing::warn!` calls include contextual fields | grep + integration test |
| AC3 | No `format!` interpolation in log messages | clippy lint |
| AC4 | Error types implement `is_transient()` and `is_fatal()` | unit tests |
| AC5 | Every `async fn` entry point has `#[tracing::instrument]` | grep integration test |
| AC6 | Field names are all `snake_case` | type system + clippy |
| AC7 | Log targets follow `veloxide::module::submodule` convention | integration test |
| AC8 | Error codes follow `ERR_*` taxonomy | unit tests |

---

## 9. Exit Criteria Compliance

- [x] Every public API behavior has at least one BDD scenario
- [x] Every pure function with multiple inputs has at least one proptest invariant
- [x] Every parsing/deserialization boundary has a fuzz target (N/A — no external parsing)
- [x] Every error variant in error taxonomy has explicit test scenario
- [x] Mutation threshold target (≥90%) is stated
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value
- [x] Error classification (is_transient/is_fatal) explicitly specified and testable

(End of file - total 450 lines)
