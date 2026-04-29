## Contract: Rate Limiter Token Bucket

### 1. Purpose

Defines the contract for the token bucket rate limiter in the veloxide circuit breaker system. This contract establishes the types, invariants, and error taxonomy for per-key rate limiting with burst capacity, sustained replenishment rates, sliding window accumulation, and fair queuing support.

### 2. Source ADRs

- `docs/adr/v2/ADR-026-v2-ai-loop-poisoning-circuit-breakers.md` (rate limiting context, circuit breaker baseline)

### 3. Rate Limiting Strategies

The system provides two rate limiting strategies:

#### 3.1 Cooldown-Based Rate Limiting (Simple)

A simple per-workflow registration rate limiter based on elapsed time.

**Pure Functions:**
```rust
/// Check if a workflow is within its rate limit window.
///
/// Returns:
/// - `None` if no active rate limit (registration permitted)
/// - `Some(remaining_secs)` if rate-limited (ceiling of remaining seconds)
fn check_rate_limit(
    last_registration: Option<Instant>,
    rate_limit_window: Duration,
    now: Instant,
) -> Option<u64>

/// Update the rate limiter with the current timestamp for a workflow.
/// Returns the new timestamp for storage.
fn update_rate_limit(now: Instant) -> Instant
```

#### 3.2 Token Bucket Rate Limiting (Advanced)

Per-key token bucket with burst/sustained rates, sliding window, and fair queuing.

### 4. Token Bucket Types

#### 4.1 TokenBucketConfig

Configuration for a token bucket rate limiter.

```rust
struct TokenBucketConfig {
    /// Maximum number of tokens in the bucket (burst capacity).
    burst: u64,
    /// Number of tokens added to the bucket per second (sustained rate).
    sustained_rate: f64,
    /// Number of tokens consumed per request.
    cost_per_request: u64,
}
```

**Constraints:**
- `burst >= 1`
- `sustained_rate >= 0.0`
- `cost_per_request >= 1`

**Defaults:**
```rust
impl Default for TokenBucketConfig {
    fn default() -> Self {
        Self {
            burst: 100,
            sustained_rate: 10.0,
            cost_per_request: 1,
        }
    }
}
```

#### 4.2 BucketState

Internal state for a single key's token bucket.

```rust
struct BucketState {
    tokens: f64,       // Current token count
    last_update: Instant, // Last replenishment timestamp
}
```

#### 4.3 TokenBucketRateLimiter

Main rate limiter type with per-key tracking.

```rust
pub struct TokenBucketRateLimiter {
    config: TokenBucketConfig,
    state: DashMap<String, BucketState>,
}
```

### 5. Core Operations

#### 5.1 check_and_consume

Check if a request is allowed and consume tokens if so.

```rust
fn check_and_consume(&self, key: &str, now: Instant) -> (bool, u64)
// Returns: (allowed, retry_after_secs)
// - allowed: whether the request is permitted
// - retry_after_secs: seconds to wait before retrying (0 if allowed)
```

**Algorithm:**
1. Look up or create bucket state for `key`
2. Replenish tokens based on elapsed time since last update
3. If `tokens >= cost_per_request`: consume tokens, return `(true, 0)`
4. Otherwise: return `(false, time_until_tokens)`

#### 5.2 peek_tokens (Fair Queuing)

Try to acquire tokens without consuming them.

```rust
fn peek_tokens(&self, key: &str, now: Instant) -> f64
// Returns: number of tokens available after replenishment (does not modify state)
```

#### 5.3 available_tokens

Get the number of tokens available for a key without modifying state.

```rust
fn available_tokens(&self, key: &str, now: Instant) -> f64
```

#### 5.4 wait_time

Get estimated wait time in seconds until enough tokens are available.

```rust
fn wait_time(&self, key: &str, now: Instant) -> u64
// Returns: 0 if tokens available, otherwise seconds until tokens replenished
```

#### 5.5 reset

Reset the token bucket for a specific key.

```rust
fn reset(&self, key: &str)
// Removes all state for the key; next request starts with full burst
```

#### 5.6 key_count

Get the number of keys currently being tracked.

```rust
fn key_count(&self) -> usize
```

### 6. Invariants (INV-*)

#### Token Bucket Core Invariants

- **INV-TB001**: New keys start with full burst capacity (`burst` tokens)
- **INV-TB002**: Burst capacity is never exceeded (`tokens <= burst` at all times)
- **INV-TB003**: Tokens accumulate at exactly `sustained_rate` tokens per second (sliding window)
- **INV-TB004**: Each request consumes exactly `cost_per_request` tokens when allowed
- **INV-TB005**: Per-key tracking is independent: exhausting one key does not affect other keys
- **INV-TB006**: `sustained_rate = 0` means no replenishment (bucket drains, never refills)
- **INV-TB007**: `reset(key)` removes all state for that key; next access creates fresh state
- **INV-TB008**: `peek_tokens` does not modify bucket state (pure observation)
- **INV-TB009**: `check_and_consume` and `peek_tokens` produce consistent results at same timestamp

#### Time Calculation Invariants

- **INV-TB010**: `wait_time` returns 0 when `available_tokens >= cost_per_request`
- **INV-TB011**: `wait_time` returns `ceil(needed / sustained_rate)` when tokens insufficient
- **INV-TB012**: If `sustained_rate <= 0`, `wait_time` returns `u64::MAX` (infinite wait)

#### State Consistency Invariants

- **INV-TB013**: `key_count` equals the number of unique keys in internal state map
- **INV-TB014**: After `reset(key)`, subsequent `available_tokens(key)` returns full burst
- **INV-TB015**: `check_and_consume` on new key creates bucket with `burst - cost_per_request` tokens

### 7. Error Taxonomy

The token bucket rate limiter does not return errors—it returns a denial with retry information.

#### 7.1 Denial Result

```rust
struct RateLimitDenied {
    key: String,
    retry_after_secs: u64,
    tokens_available: f64,
    tokens_requested: u64,
}
```

#### 7.2 Error Categories

| Condition | Category | Recoverable | retry_after_secs |
|-----------|----------|-------------|------------------|
| Insufficient tokens | LimitViolation | Yes (wait) | Calculated |
| `sustained_rate = 0` (no replenishment) | ConfigurationError | Yes (configure rate) | u64::MAX |

#### 7.3 Display Format

- Denial: "rate limit denied for key '{key}': {tokens_available:.1} tokens available, {tokens_requested} requested, retry in {retry_after_secs}s"

### 8. Enforcement Protocol

1. **Initialize**: Create `TokenBucketRateLimiter` with `TokenBucketConfig`
2. **Check and Consume**: For each request, call `check_and_consume(key, now)`
   - If `(true, 0)`: request allowed, tokens consumed
   - If `(false, secs)`: request denied, wait `secs` before retry
3. **Query Only** (Fair Queuing): Use `peek_tokens(key, now)` to check position without consuming
4. **Reset**: Call `reset(key)` to clear state for a key (e.g., on workflow completion)

### 9. Constraints

- **Thread Safety**: `TokenBucketRateLimiter` uses `DashMap` for concurrent access; no external synchronization required
- **Memory**: State grows linearly with unique keys; use `reset()` to release
- **Time Dependency**: All operations accept explicit `now: Instant` for testability; production code uses `Instant::now()`
- **No Async**: All operations are synchronous; no async variants
- **No Persistence**: State is in-memory only; loss of state on restart is acceptable for rate limiting
- **Monotonic Time**: `Instant::now()` is assumed to be monotonic; behavior with clock skew is undefined
- **Floating Point**: Token counts use `f64`; small rounding errors (< 0.001) are acceptable in assertions

### 10. Test Coverage Requirements

| Test ID | Description |
|---------|-------------|
| TB-01 | New key starts with full burst |
| TB-02 | Burst capacity is respected |
| TB-03 | Sustained rate replenishes tokens over time |
| TB-04 | Per-key tracking works independently |
| TB-05 | Sliding window - tokens accumulate smoothly |
| TB-06 | Cost per request is respected |
| TB-07 | Reset clears the bucket |
| TB-08 | Key count tracking |
| TB-09 | Zero sustained rate means no replenishment |
| TB-10 | Available tokens returns correct values |
| TB-11 | Wait time calculation |
| TB-12 | Fair queuing - peek without consuming |

### 11. Relevant Files

- `crates/vo-core/src/circuit_breaker/rate_limiter.rs` (primary implementation)
- `crates/vo-core/proptest-regressions/circuit_breaker/rate_limiter.txt` (property tests)

### 12. Acceptance Criteria

- [ ] All types (`TokenBucketConfig`, `BucketState`, `TokenBucketRateLimiter`) compile and are well-formed
- [ ] All invariants (INV-TB001 through INV-TB015) are formally stated and testable
- [ ] `check_and_consume` correctly replenishes and consumes tokens
- [ ] `peek_tokens` does not modify state (fair queuing)
- [ ] Per-key isolation is verified
- [ ] Zero sustained rate is handled correctly (no replenishment)
- [ ] `reset` properly clears bucket state
- [ ] `wait_time` calculation is correct
- [ ] Contract is self-contained and references existing implementation only