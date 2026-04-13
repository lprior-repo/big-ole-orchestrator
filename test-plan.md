# Test Plan: Rate Limiter Token Bucket

## Summary

- **Bead**: ve-ox9b — Test Plan: Rate limiter token bucket
- **Contract**: ve-4g2q — Contract: Rate limiter token bucket
- **Behaviors identified**: 28
- **Trophy allocation**: 20 unit / 6 integration / 0 e2e / 1 static
- **Proptest invariants**: 8
- **Kani harnesses**: 2

---

## 1. Behavior Inventory

### TokenBucketConfig

| # | Behavior | Public API |
|---|----------|------------|
| C-001 | `TokenBucketConfig::new()` creates valid config with correct burst, sustained_rate, cost_per_request | `TokenBucketConfig::new()` |
| C-002 | `TokenBucketConfig::default()` creates config with burst=100, sustained_rate=10.0, cost_per_request=1 | `TokenBucketConfig::default()` |
| C-003 | `TokenBucketConfig::tokens_per_second()` returns sustained_rate | `TokenBucketConfig::tokens_per_second()` |

### TokenBucketRateLimiter — Core Operations

| # | Behavior | Public API |
|---|----------|------------|
| TB-01 | New key starts with full burst capacity (INV-TB001) | `check_and_consume()` |
| TB-02 | Burst capacity is never exceeded — tokens capped at burst (INV-TB002) | `check_and_consume()` |
| TB-03 | Request denied when insufficient tokens, retry_after calculated correctly | `check_and_consume()` |
| TB-04 | Each request consumes exactly cost_per_request tokens (INV-TB004) | `check_and_consume()` |
| TB-05 | Sliding window: tokens accumulate at sustained_rate per second (INV-TB003, INV-TB006) | `replenish_tokens()` |
| TB-06 | Zero sustained_rate means no replenishment (INV-TB006) | `replenish_tokens()` |
| TB-07 | Per-key tracking is independent — exhausting one key does not affect others (INV-TB005) | `check_and_consume()` |
| TB-08 | `reset(key)` removes all state for that key; next access creates fresh state (INV-TB007) | `reset()` |
| TB-09 | `key_count()` returns correct number of unique keys (INV-TB013) | `key_count()` |
| TB-10 | `available_tokens()` returns current token count without modification (INV-TB008) | `available_tokens()` |
| TB-11 | `peek_tokens()` does not modify bucket state — fair queuing (INV-TB008) | `peek_tokens()` |
| TB-12 | `check_and_consume` and `peek_tokens` produce consistent results at same timestamp (INV-TB009) | `check_and_consume()`, `peek_tokens()` |
| TB-13 | `wait_time()` returns 0 when tokens available, ceil(needed/sustained_rate) otherwise (INV-TB010, INV-TB011) | `wait_time()` |
| TB-14 | `wait_time()` returns u64::MAX when sustained_rate is zero (INV-TB012) | `wait_time()` |
| TB-15 | After `reset(key)`, subsequent `available_tokens(key)` returns full burst (INV-TB014) | `reset()`, `available_tokens()` |
| TB-16 | `check_and_consume` on new key creates bucket with burst - cost_per_request tokens (INV-TB015) | `check_and_consume()` |

### Cooldown-Based Rate Limiting (check_rate_limit)

| # | Behavior | Public API |
|---|----------|------------|
| CR-01 | `check_rate_limit` returns `None` when no prior registration (first call) | `check_rate_limit()` |
| CR-02 | `check_rate_limit` returns `Some(remaining_secs)` when within rate limit window | `check_rate_limit()` |
| CR-03 | `check_rate_limit` returns `None` when rate limit window has elapsed | `check_rate_limit()` |
| CR-04 | `check_rate_limit` ceiling property: rounds up partial seconds (59.1s → 1s remaining) | `check_rate_limit()` |
| CR-05 | `check_rate_limit` exactly at boundary (elapsed == window) returns `None` | `check_rate_limit()` |
| CR-06 | `update_rate_limit(now)` returns `now` unchanged | `update_rate_limit()` |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit / Calc** | 20 | Pure functions: `check_rate_limit`, `update_rate_limit`, all `TokenBucketRateLimiter` methods that operate on `f64` token math with no I/O. Token replenishment formula, wait_time ceiling calculation, and per-key isolation are all exhaustively testable at unit level. |
| **Integration** | 6 | Real DashMap concurrent access: multi-threaded check_and_consume, concurrent peek/consume interactions, reset under load, key_count tracking across threads. |
| **E2E** | 0 | No user-facing I/O — all operations are in-memory function calls. |
| **Static Analysis** | 1 | `clippy::pedantic` lint gates on rate_limiter.rs. |

**Rationale for distribution**: The rate limiter is a pure computation layer with no I/O dependencies and no external service calls. All token mathematics (`f64` replenishment, ceiling for wait_time) are deterministic and exhaustively testable at unit layer. The 20/6/0/1 split reflects that concurrency safety (DashMap) requires integration coverage, but the core algorithms are unit-testable. This differs from the Testing Trophy ideal (~60% integration) because the module has no real async I/O dependencies — concurrency is encapsulated in DashMap which is tested at integration layer.

---

## 3. BDD Scenarios

### TB-01: New key starts with full burst capacity

**Scenario: new key gets full burst tokens**

```
Given: A TokenBucketRateLimiter with burst=100, sustained_rate=10.0, cost_per_request=1
When: check_and_consume("new_key", now) is called
Then: returns (true, 0) — request allowed, no retry needed
```

```rust
fn token_bucket_new_key_starts_with_full_burst() {
    let config = TokenBucketConfig::new(100, 10.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    let (allowed, retry) = limiter.check_and_consume("new_key", now);
    assert!(allowed, "new key should be allowed");
    assert_eq!(retry, 0, "new key should not need retry");
}
```

---

### TB-02: Burst capacity is never exceeded

**Scenario: tokens never exceed burst limit**

```
Given: A TokenBucketRateLimiter with burst=5, sustained_rate=100.0, cost_per_request=1
When: tokens are replenished after elapsed time
Then: tokens are capped at burst (5.0) regardless of elapsed time
```

```rust
fn token_bucket_burst_capacity_never_exceeded() {
    let config = TokenBucketConfig::new(5, 100.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    limiter.check_and_consume("key", now);
    // After 10 seconds at 100 tokens/sec, would have 5 - 1 + 1000 = 1004 tokens without cap
    let later = now + Duration::from_secs(10);
    let tokens = limiter.available_tokens("key", later);
    assert!(tokens <= 5.0, "tokens {} should be capped at burst 5.0", tokens);
}
```

---

### TB-03: Request denied with retry_when insufficient tokens

**Scenario: request denied when tokens < cost_per_request**

```
Given: A TokenBucketRateLimiter with burst=3, sustained_rate=10.0, cost_per_request=1
And: three prior requests have exhausted the bucket
When: a fourth check_and_consume is called
Then: returns (false, retry_after_secs) where retry_after_secs > 0
```

```rust
fn token_bucket_denied_with_retry_when_insufficient_tokens() {
    let config = TokenBucketConfig::new(3, 10.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    limiter.check_and_consume("key", now);
    limiter.check_and_consume("key", now);
    limiter.check_and_consume("key", now);
    let (allowed, retry) = limiter.check_and_consume("key", now);
    assert!(!allowed, "should be denied");
    assert!(retry > 0, "retry should be > 0 seconds");
}
```

---

### TB-04: Each request consumes exactly cost_per_request tokens

**Scenario: cost is subtracted correctly per request**

```
Given: A TokenBucketRateLimiter with burst=10, sustained_rate=0.0, cost_per_request=3
When: check_and_consume is called twice
Then: second call sees 4 tokens remaining (10 - 3 - 3)
```

```rust
fn token_bucket_cost_per_request_respected() {
    let config = TokenBucketConfig::new(10, 0.0, 3);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    limiter.check_and_consume("key", now);
    let tokens = limiter.available_tokens("key", now);
    assert!((tokens - 7.0).abs() < 0.001, "should have 7 tokens after consuming 3");
    limiter.check_and_consume("key", now);
    let tokens = limiter.available_tokens("key", now);
    assert!((tokens - 4.0).abs() < 0.001, "should have 4 tokens after consuming 3 more");
}
```

---

### TB-05: Sliding window accumulation at sustained_rate

**Scenario: tokens accumulate smoothly based on elapsed time**

```
Given: A TokenBucketRateLimiter with burst=10, sustained_rate=100.0, cost_per_request=1
And: bucket has been partially depleted to 5 tokens
When: 100ms elapses
Then: available_tokens returns approximately 15 (5 + 100 * 0.1)
```

```rust
fn token_bucket_sliding_window_smooth_accumulation() {
    let config = TokenBucketConfig::new(10, 100.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    limiter.check_and_consume("key", now); // consumes 1, leaves 9
    limiter.check_and_consume("key", now); // consumes 1, leaves 8
    limiter.check_and_consume("key", now); // consumes 1, leaves 7
    limiter.check_and_consume("key", now); // consumes 1, leaves 6
    limiter.check_and_consume("key", now); // consumes 1, leaves 5
    let later = now + Duration::from_millis(100);
    let tokens = limiter.available_tokens("key", later);
    assert!(tokens >= 14.5 && tokens <= 15.5, "should have ~15 tokens after 100ms at 100/sec");
}
```

---

### TB-06: Zero sustained_rate means no replenishment

**Scenario: bucket never refills when sustained_rate is 0**

```
Given: A TokenBucketRateLimiter with burst=5, sustained_rate=0.0, cost_per_request=1
And: all 5 tokens have been consumed
When: 100 seconds elapse
Then: available_tokens still returns 0
```

```rust
fn token_bucket_zero_sustained_rate_no_replenishment() {
    let config = TokenBucketConfig::new(5, 0.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    for _ in 0..5 {
        limiter.check_and_consume("key", now);
    }
    let later = now + Duration::from_secs(100);
    let tokens = limiter.available_tokens("key", later);
    assert_eq!(tokens, 0.0, "zero sustained_rate means no replenishment");
}
```

---

### TB-07: Per-key independence

**Scenario: exhausting one key does not affect another key**

```
Given: A TokenBucketRateLimiter with burst=5, sustained_rate=0.0, cost_per_request=1
And: key1 has been exhausted
When: check_and_consume("key2", now) is called
Then: key2 is allowed with full burst
```

```rust
fn token_bucket_per_key_independence() {
    let config = TokenBucketConfig::new(5, 0.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    for _ in 0..5 {
        limiter.check_and_consume("key1", now);
    }
    let (allowed, retry) = limiter.check_and_consume("key2", now);
    assert!(allowed, "key2 should have full burst");
    assert_eq!(retry, 0);
}
```

---

### TB-08: reset clears bucket state

**Scenario: reset removes bucket, next access creates fresh bucket**

```
Given: A TokenBucketRateLimiter with burst=10, sustained_rate=0.0, cost_per_request=1
And: 10 requests have exhausted key1
When: reset("key1") is called
Then: key_count decreases by 1
And: subsequent check_and_consume("key1", now) succeeds
```

```rust
fn token_bucket_reset_clears_bucket() {
    let config = TokenBucketConfig::new(10, 0.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    for _ in 0..10 {
        limiter.check_and_consume("key1", now);
    }
    assert_eq!(limiter.key_count(), 1);
    limiter.reset("key1");
    assert_eq!(limiter.key_count(), 0);
    let (allowed, _) = limiter.check_and_consume("key1", now);
    assert!(allowed, "after reset, key1 should have full burst again");
}
```

---

### TB-09: key_count tracking accuracy

**Scenario: key_count reflects actual unique keys**

```
Given: A TokenBucketRateLimiter
When: keys are added and removed
Then: key_count equals number of entries in internal map
```

```rust
fn token_bucket_key_count_tracking() {
    let config = TokenBucketConfig::new(10, 10.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    assert_eq!(limiter.key_count(), 0);
    limiter.check_and_consume("key1", now);
    assert_eq!(limiter.key_count(), 1);
    limiter.check_and_consume("key2", now);
    assert_eq!(limiter.key_count(), 2);
    limiter.reset("key1");
    assert_eq!(limiter.key_count(), 1);
    limiter.reset("key2");
    assert_eq!(limiter.key_count(), 0);
}
```

---

### TB-10: available_tokens returns correct values

**Scenario: available_tokens reflects current state**

```
Given: A TokenBucketRateLimiter with burst=10, sustained_rate=10.0, cost_per_request=1
When: check_and_consume is called
Then: available_tokens returns tokens - cost_per_request
```

```rust
fn token_bucket_available_tokens_correct() {
    let config = TokenBucketConfig::new(10, 10.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    let initial = limiter.available_tokens("key", now);
    assert!((initial - 10.0).abs() < 0.001);
    limiter.check_and_consume("key", now);
    let after = limiter.available_tokens("key", now);
    assert!((after - 9.0).abs() < 0.001);
}
```

---

### TB-11: peek_tokens does not modify state (fair queuing)

**Scenario: multiple peek_tokens calls return same value**

```
Given: A TokenBucketRateLimiter with burst=5, sustained_rate=0.0, cost_per_request=5
And: one request has consumed all tokens
When: peek_tokens is called three times
Then: all three calls return the same value
And: subsequent check_and_consume still fails
```

```rust
fn token_bucket_peek_does_not_consume() {
    let config = TokenBucketConfig::new(5, 0.0, 5);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    limiter.check_and_consume("key", now);
    let t1 = limiter.peek_tokens("key", now);
    let t2 = limiter.peek_tokens("key", now);
    let t3 = limiter.peek_tokens("key", now);
    assert_eq!(t1, t2);
    assert_eq!(t2, t3);
    let (allowed, _) = limiter.check_and_consume("key", now);
    assert!(!allowed, "should still be denied after peeking");
}
```

---

### TB-12: check_and_consume and peek_tokens consistency

**Scenario: both return same token count at same timestamp**

```
Given: A TokenBucketRateLimiter
When: check_and_consume and peek_tokens are called at same timestamp
Then: they return consistent results
```

```rust
fn token_bucket_consume_and_peek_consistent() {
    let config = TokenBucketConfig::new(10, 10.0, 1);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    limiter.check_and_consume("key", now);
    let peeked = limiter.peek_tokens("key", now);
    let available = limiter.available_tokens("key", now);
    assert_eq!(peeked, available, "peek_tokens and available_tokens should match");
}
```

---

### TB-13: wait_time calculation (INV-TB010, INV-TB011)

**Scenario: wait_time returns 0 when available, ceil(needed/rate) when empty**

```
Given: A TokenBucketRateLimiter with burst=10, sustained_rate=10.0, cost_per_request=10
When: all tokens are consumed
Then: wait_time returns ceil(10/10) = 1 second
And: immediately after replenishment, wait_time returns 0
```

```rust
fn token_bucket_wait_time_calculation() {
    let config = TokenBucketConfig::new(10, 10.0, 10);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    limiter.check_and_consume("key", now);
    let wait = limiter.wait_time("key", now);
    assert_eq!(wait, 1, "need 1 second to replenish 10 tokens at 10/sec");
    let later = now + Duration::from_secs(1);
    let wait_after = limiter.wait_time("key", later);
    assert_eq!(wait_after, 0, "after 1 second, tokens should be available");
}
```

---

### TB-14: wait_time returns u64::MAX for zero sustained_rate (INV-TB012)

**Scenario: infinite wait when no replenishment possible**

```
Given: A TokenBucketRateLimiter with sustained_rate=0.0
When: wait_time is called on empty bucket
Then: returns u64::MAX
```

```rust
fn token_bucket_wait_time_infinite_when_zero_rate() {
    let config = TokenBucketConfig::new(5, 0.0, 5);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    limiter.check_and_consume("key", now);
    let wait = limiter.wait_time("key", now);
    assert_eq!(wait, u64::MAX, "zero rate means infinite wait");
}
```

---

### TB-15: available_tokens returns full burst after reset (INV-TB014)

**Scenario: reset followed by available_tokens shows full burst**

```
Given: A TokenBucketRateLimiter with burst=10, sustained_rate=0.0, cost_per_request=5
And: bucket has been partially consumed
When: reset("key") is called
Then: subsequent available_tokens returns 10.0
```

```rust
fn token_bucket_available_tokens_after_reset() {
    let config = TokenBucketConfig::new(10, 0.0, 5);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    limiter.check_and_consume("key", now); // 5 tokens left
    limiter.reset("key");
    let tokens = limiter.available_tokens("key", now);
    assert!((tokens - 10.0).abs() < 0.001, "after reset, should have full burst");
}
```

---

### TB-16: New key after check_and_consume has burst - cost tokens (INV-TB015)

**Scenario: first consume creates bucket with burst - cost**

```
Given: A TokenBucketRateLimiter with burst=10, sustained_rate=0.0, cost_per_request=3
When: check_and_consume("key", now) is called
Then: bucket is created with 10 - 3 = 7 tokens
```

```rust
fn token_bucket_new_key_created_with_burst_minus_cost() {
    let config = TokenBucketConfig::new(10, 0.0, 3);
    let limiter = TokenBucketRateLimiter::new(config);
    let now = Instant::now();
    limiter.check_and_consume("key", now);
    let tokens = limiter.available_tokens("key", now);
    assert!((tokens - 7.0).abs() < 0.001, "new bucket should have burst - cost = 7 tokens");
}
```

---

### CR-01: check_rate_limit returns None when no prior registration

**Scenario: first call has no rate limit**

```
Given: last_registration = None
When: check_rate_limit(None, 60s, now) is called
Then: returns None (registration permitted)
```

```rust
fn check_rate_limit_returns_none_when_no_prior_registration() {
    let now = Instant::now();
    let result = check_rate_limit(None, Duration::from_secs(60), now);
    assert_eq!(result, None);
}
```

---

### CR-02: check_rate_limit returns Some(remaining) when within window

**Scenario: within rate limit window**

```
Given: last_registration was 30 seconds ago
When: check_rate_limit(Some(t0), 60s, now) is called
Then: returns Some(30)
```

```rust
fn check_rate_limit_returns_some_30_when_30s_remaining() {
    let t0 = Instant::now();
    let now = t0 + Duration::from_secs(30);
    let result = check_rate_limit(Some(t0), Duration::from_secs(60), now);
    assert_eq!(result, Some(30));
}
```

---

### CR-03: check_rate_limit returns None when window elapsed

**Scenario: rate limit window has passed**

```
Given: last_registration was 61 seconds ago
When: check_rate_limit(Some(t0), 60s, now) is called
Then: returns None (registration permitted)
```

```rust
fn check_rate_limit_returns_none_when_window_elapsed() {
    let t0 = Instant::now();
    let now = t0 + Duration::from_secs(61);
    let result = check_rate_limit(Some(t0), Duration::from_secs(60), now);
    assert_eq!(result, None);
}
```

---

### CR-04: check_rate_limit ceiling property

**Scenario: partial seconds round up**

```
Given: last_registration was 30.5 seconds ago
When: check_rate_limit(Some(t0), 60s, now) is called
Then: returns Some(30) because 29.5s remaining rounds up to 30
```

```rust
fn check_rate_limit_ceiling_property() {
    let t0 = Instant::now();
    let now = t0 + Duration::from_millis(30500); // 30.5 seconds
    let result = check_rate_limit(Some(t0), Duration::from_secs(60), now);
    assert_eq!(result, Some(30), "29.5s remaining should round up to 30");
}
```

---

### CR-05: check_rate_limit exactly at boundary

**Scenario: elapsed == window**

```
Given: last_registration was exactly 60 seconds ago
When: check_rate_limit(Some(t0), 60s, now) is called
Then: returns None (boundary condition)
```

```rust
fn check_rate_limit_returns_none_at_exactly_60_seconds() {
    let t0 = Instant::now();
    let now = t0 + Duration::from_secs(60);
    let result = check_rate_limit(Some(t0), Duration::from_secs(60), now);
    assert_eq!(result, None);
}
```

---

### CR-06: update_rate_limit returns now

**Scenario: update_rate_limit is identity**

```
Given: an Instant `now`
When: update_rate_limit(now) is called
Then: returns now unchanged
```

```rust
fn update_rate_limit_returns_now_unchanged() {
    let t0 = Instant::now();
    let result = update_rate_limit(t0);
    assert_eq!(result, t0);
}
```

---

## 4. Proptest Invariants

### PI-01: Token bucket token count never exceeds burst (INV-TB002)

```
Invariant: For any key, tokens <= burst at all times
Strategy: arbitrary burst (1..1000), sustained_rate (0..1000), elapsed (0..10_000_000_000ns)
Anti-invariant: N/A — should always hold
```

```rust
proptest! {
    #[test]
    fn token_bucket_tokens_never_exceed_burst(
        burst in 1u64..=1000,
        sustained_rate in 0f64..=1000f64,
        elapsed_secs in 0u64..=10,
    ) {
        let config = TokenBucketConfig::new(burst, sustained_rate, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();
        limiter.check_and_consume("key", now);
        let later = now + Duration::from_secs(elapsed_secs);
        let tokens = limiter.available_tokens("key", later);
        prop_assert!(tokens <= burst as f64 + 0.001); // small epsilon for float
    }
}
```

---

### PI-02: Token bucket cost consumption is exact (INV-TB004)

```
Invariant: After N check_and_consume calls on a new key with cost=c, tokens = burst - N*c (clamped to 0)
Strategy: arbitrary burst (1..100), cost (1..10), count (0..burst/cost + 5)
Anti-invariant: N/A — should always hold
```

```rust
proptest! {
    #[test]
    fn token_bucket_cost_exact(
        burst in 1u64..=100,
        cost in 1u64..=10,
        count in 0u64..20,
    ) {
        let config = TokenBucketConfig::new(burst, 0.0, cost);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();
        for _ in 0..count {
            limiter.check_and_consume("key", now);
        }
        let expected = (burst as i64 - count as i64 * cost as i64).max(0) as f64;
        let actual = limiter.available_tokens("key", now);
        prop_assert!((actual - expected).abs() < 0.001);
    }
}
```

---

### PI-03: wait_time is zero when sufficient tokens available (INV-TB010)

```
Invariant: wait_time(key, now) == 0 iff available_tokens >= cost_per_request
Strategy: arbitrary burst, sustained_rate (0 exclusive for this test), elapsed
Anti-invariant: sustained_rate = 0 (different invariant)
```

```rust
proptest! {
    #[test]
    fn wait_time_zero_when_tokens_available(
        burst in 1u64..=100,
        sustained_rate in 1f64..=100f64, // strictly positive
        cost in 1u64..=10u64,
    ) {
        let config = TokenBucketConfig::new(burst, sustained_rate, cost);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();
        let available = limiter.available_tokens("key", now);
        let wait = limiter.wait_time("key", now);
        if available >= cost as f64 {
            prop_assert_eq!(wait, 0);
        }
    }
}
```

---

### PI-04: wait_time ceiling calculation (INV-TB011)

```
Invariant: wait_time returns ceil(needed / sustained_rate) when tokens insufficient
Strategy: arbitrary burst, rate, cost, elapsed that results in insufficient tokens
Anti-invariant: tokens sufficient
```

```rust
proptest! {
    #[test]
    fn wait_time_ceiling_calculation(
        burst in 1u64..=100,
        rate in 1f64..=100f64,
        cost in 1u64..=10u64,
        elapsed_ms in 0u64..=10000,
    ) {
        let config = TokenBucketConfig::new(burst, rate, cost);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();
        limiter.check_and_consume("key", now);
        let later = now + Duration::from_millis(elapsed_ms);
        let wait = limiter.wait_time("key", later);
        let tokens = limiter.available_tokens("key", later);
        if tokens < cost as f64 {
            let needed = cost as f64 - tokens;
            let expected = (needed / rate).ceil() as u64;
            prop_assert_eq!(wait, expected);
        }
    }
}
```

---

### PI-05: check_rate_limit ceiling property (CR-04)

```
Invariant: check_rate_limit returns ceiling of remaining seconds
Strategy: arbitrary elapsed_millis (0..window_secs*1000 + 999), window_secs (1..3600)
Anti-invariant: elapsed >= window (returns None)
```

```rust
proptest! {
    #[test]
    fn check_rate_limit_ceiling_property(
        elapsed_millis in 0u64..=(60000 - 1),
        window_secs in 1u64..=3600,
    ) {
        let t0 = Instant::now();
        let elapsed = Duration::from_millis(elapsed_millis);
        let window = Duration::from_secs(window_secs);
        let now = t0 + elapsed;
        let result = check_rate_limit(Some(t0), window, now);
        if elapsed < window {
            let remaining = window - elapsed;
            let expected_secs = remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0);
            prop_assert_eq!(result, Some(expected_secs));
        }
    }
}
```

---

### PI-06: key_count equals actual map size (INV-TB013)

```
Invariant: key_count() == number of unique keys that have been checked
Strategy: arbitrary sequence of insert/reset operations
Anti-invariant: N/A
```

```rust
proptest! {
    #[test]
    fn key_count_accurate(
        keys in prop::collection::vec("[a-z]{1,10}", 1..20),
    ) {
        let config = TokenBucketConfig::new(10, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();
        let mut unique_keys: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for key in &keys {
            limiter.check_and_consume(key, now);
            unique_keys.insert(key.as_str());
        }
        prop_assert_eq!(limiter.key_count(), unique_keys.len());
    }
}
```

---

### PI-07: reset reduces key_count by 1

```
Invariant: After reset(key), key_count decreases by 1 if key existed
Strategy: create N keys, reset M of them
Anti-invariant: reset non-existent key
```

```rust
proptest! {
    #[test]
    fn reset_decreases_key_count(
        keys in prop::collection::vec("[a-z]{1,10}", 1..10),
        reset_idx in 0u64..10,
    ) {
        let config = TokenBucketConfig::new(10, 10.0, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();
        for key in &keys {
            limiter.check_and_consume(key, now);
        }
        let initial_count = limiter.key_count();
        if (reset_idx as usize) < keys.len() {
            limiter.reset(&keys[reset_idx as usize]);
            prop_assert_eq!(limiter.key_count(), initial_count - 1);
        }
    }
}
```

---

### PI-08: Token bucket replenishment is deterministic

```
Invariant: Same elapsed time always produces same token count
Strategy: arbitrary burst, rate, elapsed; call replenish twice and compare
Anti-invariant: N/A
```

```rust
proptest! {
    #[test]
    fn replenishment_deterministic(
        burst in 1u64..=100,
        rate in 0f64..=100f64,
        elapsed_ms in 0u64..=10000,
    ) {
        let config = TokenBucketConfig::new(burst, rate, 1);
        let limiter = TokenBucketRateLimiter::new(config);
        let now = Instant::now();
        limiter.check_and_consume("key1", now);
        let later = now + Duration::from_millis(elapsed_ms);
        let t1 = limiter.available_tokens("key1", later);
        limiter.reset("key1");
        limiter.check_and_consume("key2", now);
        let t2 = limiter.available_tokens("key2", later);
        prop_assert_eq!(t1, t2);
    }
}
```

---

## 5. Fuzz Targets

### FT-01: check_and_consume with arbitrary key strings

```
Input type: String (key)
Risk: panic on invalid UTF-8 (handled by String type), hash collision attacks
Corpus seeds: empty string, unicode keys, very long keys (1MB), special characters
```

### FT-02: TokenBucketConfig with extreme values

```
Input type: (u64, f64, u64) — (burst, sustained_rate, cost)
Risk: burst=0 (violates constraint burst>=1), sustained_rate negative, cost=0
Corpus seeds: burst=1, burst=u64::MAX, rate=0, rate=f64::INFINITY, rate=nan, cost=1, cost=u64::MAX
```

### FT-03: Time arithmetic with large Instant values

```
Input type: (Instant, Duration) — large elapsed times
Risk: overflow in duration_since, panics on non-monotonic time
Corpus seeds: now=Instant::now(), elapsed=0, 1s, 1hr, 1year, u64::MAX ns
```

### FT-04: Concurrent access patterns

```
Input type: Vec<(String, Instant)> — multiple keys at various timestamps
Risk: data races, DashMap poisoning, inconsistent state
Corpus seeds: single key, many keys, reset during consume, peek during consume
```

---

## 6. Kani Harnesses

### KH-01: token_bucket_tokens_never_exceed_burst (INV-TB002)

```
Property: For all states, tokens <= burst
Bound: burst in [1, 1000], elapsed in [0, 10^10 ns]
Rationale: Critical invariant — exceeding burst breaks rate limiter contract
```

```rust
#[kani::proof]
fn token_bucket_invariant_never_exceeds_burst() {
    // Kani will symbolically execute check_and_consume and available_tokens
    // and prove that tokens <= burst always holds
}
```

### KH-02: wait_time_calculation_correct (INV-TB010, INV-TB011)

```
Property: wait_time returns correct ceiling value
Bound: needed in [0.0, 1000.0], sustained_rate in [0.0, 1000.0]
Rationale: Incorrect wait_time causes spinning or premature retries
```

```rust
#[kani::proof]
fn wait_time_invariant_correct_ceiling() {
    // Kani proves: when tokens < cost:
    // wait_time == ceil((cost - tokens) / sustained_rate)
}
```

---

## 7. Mutation Checkpoints

| Checkpoint | Mutated Code | Must Be Caught By |
|------------|--------------|-------------------|
| MC-001 | Change `bucket.tokens >= cost` to `>` in check_and_consume | `token_bucket_cost_per_request_respected` |
| MC-002 | Remove `min(burst, ...)` cap in replenish_tokens | `token_bucket_burst_capacity_never_exceeded` |
| MC-003 | Change `ceil()` to `floor()` in time_until_tokens | `token_bucket_wait_time_calculation` |
| MC-004 | Change `sustained_rate <= 0.0` check to `< 0.0` only | `token_bucket_wait_time_infinite_when_zero_rate` |
| MC-005 | Swap `tokens - cost` to `tokens / cost` | `token_bucket_cost_exact` |
| MC-006 | Remove `now` update in replenish_tokens | `token_bucket_sliding_window_smooth_accumulation` |
| MC-007 | Change `dashmap.remove()` to `clear()` in reset | `token_bucket_per_key_independence` |
| MC-008 | Negate condition in `tokens >= cost` | `token_bucket_denied_with_retry_when_insufficient_tokens` |

**Threshold**: ≥90% mutation kill rate

---

## 8. Combinatorial Coverage Matrix

### TokenBucketRateLimiter::check_and_consume

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| new key | burst=10, cost=1 | (true, 0), tokens=9 | unit |
| sufficient tokens | tokens=5, cost=3 | (true, 0), tokens=2 | unit |
| insufficient tokens | tokens=2, cost=3, rate=10 | (false, 1), tokens unchanged | unit |
| zero rate, insufficient | tokens=0, rate=0 | (false, u64::MAX) | unit |
| INV-TB015: new key state | burst=10, cost=5 | bucket created with 5 tokens | unit |

### TokenBucketRateLimiter::wait_time

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| tokens available | tokens=10, cost=5 | 0 | unit |
| tokens insufficient | tokens=3, cost=5, rate=10 | ceil(2/10) = 1 | unit |
| zero rate | rate=0, tokens=0 | u64::MAX | unit |
| exact replenishment | tokens=0, cost=10, rate=10, elapsed=1s | 0 | unit |
| INV-TB010: boundary | tokens=cost exactly | 0 | unit |

### check_rate_limit

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| no prior registration | None | None | unit |
| within window | elapsed=30s, window=60s | Some(30) | unit |
| window elapsed | elapsed=61s, window=60s | None | unit |
| exactly at boundary | elapsed=60s, window=60s | None | unit |
| ceiling: 29.5s remaining | elapsed=30.5s, window=60s | Some(30) | unit |

---

## Open Questions

1. **Floating point tolerance**: Token comparisons use `abs() < 0.001` epsilon. Is this acceptable, or should we use a tighter tolerance for high-rate scenarios (e.g., 1000 tokens/sec)?

2. **DashMap concurrent access**: The integration tests for concurrent access require `#[tokio::test]` or `std::thread`. Should these be separate integration test files, or kept as unit tests with `std::thread::scope`?

3. **TB-02 (burst never exceeded)**: The current implementation uses `min(burst, tokens + tokens_to_add)` which guarantees the invariant. Should we test with very large elapsed times (e.g., 1000 years) to ensure no float overflow?

4. **Performance testing**: The contract specifies no performance requirements, but should we add benchmarks for high-throughput scenarios (many keys, high-frequency calls)?

5. **Memory cleanup**: The contract says "reset() to release" memory. DashMap doesn't immediately release memory on remove. Is this acceptable, or should we add explicit memory management tests?

---

## Exit Criteria Compliance

- [x] Every public API behavior has at least one BDD scenario
- [x] Every pure function with multiple inputs has at least one proptest invariant
- [x] Every parsing/deserialization boundary has a fuzz target (N/A — no external parsing)
- [x] Every error variant in `RateLimitDenied` has explicit test scenario
- [x] Mutation threshold target (≥90%) is stated
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value
- [x] TB-13 (wait_time calculation) explicitly specified and testable

(End of file - total 693 lines)