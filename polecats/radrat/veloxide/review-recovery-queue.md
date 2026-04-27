# BLACK-HAT Review: Recovery Queue Throttling & Orphan Detection

**Bead:** `ve-zvwe1`  
**Review Type:** Adversarial review (Go State 5.5)  
**Focus:** Starvation freedom, bounded queue invariants  
**STATUS:** REJECTED

---

## Summary

The recovery queue throttling and orphan detection implementation fails BLACK-HAT review due to **critical starvation freedom violations** and **missing bounded queue invariants**.

---

## Critical Findings

### 1. Starvation Freedom Violation

**Location:** `crates/vo-core/src/recovery/throttle.rs`

**Issue:** The token bucket implementation allows indefinite starvation when `refill_rate < enqueue_rate`.

**Evidence:**
```rust
fn try_consume(&mut self) -> bool {
    self.refill();
    if self.tokens > 0 {
        self.tokens -= 1;
        true
    } else {
        false  // Starvation: returns false, no retry guarantee
    }
}
```

**Attack Vector:**
- Configuration: `capacity=1, refill_rate=1, refill_period=1s`
- Attack: Enqueue at 2 items/second consistently
- Result: 50% of orphans are permanently lost (throttle returns `QueueFull` with no backpressure handling)

**Violation:** Requirement #3 states "IF recovery queue is full, THE SYSTEM SHALL NOT enqueue more orphans" but does NOT specify how orphans are preserved when throttled. This creates a **silent data loss** scenario.

### 2. Missing Bounded Queue Invariant Enforcement

**Location:** `crates/vo-core/src/recovery/mod.rs`

**Issue:** No mechanism to persist throttled orphans. The `RecoveryError::QueueFull` error is returned but the orphan is lost.

**Evidence:**
```rust
pub async fn enqueue(&mut self, _item: RecoveryItem) -> RecoveryResult<()> {
    if !self.bucket.try_consume() {
        return Err(RecoveryError::QueueFull);  // Orphan lost here!
    }
    Ok(())
}
```

**Attack Vector:**
1. System under high failure load
2. Recovery throttle fills to capacity
3. New orphans detected by sweep
4. Enqueue fails with `QueueFull`
5. Orphan is silently dropped
6. System never recovers the dropped orphan

**Missing Invariant:**
```
INVARIANT: Every detected orphan must eventually be processed or explicitly discarded
CURRENT: Orphans can be silently dropped when throttle returns QueueFull
```

### 3. Time Overflow Vulnerability

**Location:** `crates/vo-core/src/recovery/throttle.rs:69-73`

**Issue:** `elapsed / self.refill_period.as_millis() as u64` can overflow on long-running systems.

**Evidence:**
```rust
fn refill(&mut self) {
    if self.current_time >= self.last_refill {
        let elapsed = self.current_time - self.last_refill;  // Can overflow
        let periods = elapsed / self.refill_period.as_millis() as u64;  // Division by zero if period=0
        if periods > 0 {
            let refill_amount = (periods as usize).saturating_mul(self.refill_rate);
            self.tokens = (self.tokens + refill_amount).min(self.max_tokens);
            self.last_refill = self.current_time;
        }
    }
}
```

**Attack Vector:**
- `refill_period = Duration::from_millis(0)` → division by zero panic
- `elapsed` overflows `u64` on systems running >584 years (unlikely but possible in long-lived services)

### 4. Orphan Detector Lacks Backpressure Handling

**Location:** `crates/vo-core/src/recovery/sweep.rs:30-38`

**Issue:** The `OrphanDetector` does not handle channel backpressure when sending orphans to the throttle.

**Evidence:**
```rust
pub async fn run(self, tx: mpsc::Sender<OrphanProcess>) {
    let mut interval = interval(self.sweep_interval);
    loop {
        interval.tick().await;
        match self.query.query_orphans().await {
            Ok(orphans) => {
                for orphan in orphans {
                    if tx.send(orphan).await.is_err() {  // Only returns on drop, not on full
                        return;
                    }
                }
            }
            Err(_) => {}  // Silent error handling!
        }
    }
}
```

**Attack Vector:**
1. Orphan detection rate > throttle processing rate
2. Channel buffer fills up
3. `tx.send()` blocks indefinitely
4. Sweep thread hangs
5. No new orphans can be detected
6. System deadlock

**Violation:** Starvation freedom is broken when the channel blocks.

### 5. Silent Error Suppression

**Location:** `crates/vo-core/src/recovery/sweep.rs:37`

**Issue:** `Err(_)` from `query_orphans()` is silently ignored.

**Evidence:**
```rust
Err(_) => {}  // Silent failure!
```

**Attack Vector:**
- Storage query fails intermittently
- Orphans not detected during failures
- Silent degradation with no observability

---

## Missing Critical Tests

### 1. No Starvation Freedom Tests

**Expected:**
```rust
#[tokio::test]
async fn starvation_freedom_orphans_eventually_processed() {
    // Verify that orphans are not lost when throttle is saturated
    // for extended periods
}
```

**Actual:** No such test exists.

### 2. No Backpressure Tests

**Expected:**
```rust
#[tokio::test]
async fn backpressure_orphans_preserved_when_full() {
    // Verify orphans are persisted when throttle is full
}
```

**Actual:** No such test exists.

### 3. No Time Overflow Tests

**Expected:**
```rust
#[tokio::test]
#[should_panic]
async fn zero_refill_period_panics() {
    let config = RecoveryThrottleConfig::new(1, 1, Duration::from_millis(0));
    RecoveryThrottle::new(config);
}
```

**Actual:** No such test exists.

### 4. No Channel Deadlock Tests

**Expected:**
```rust
#[tokio::test]
async fn sweep_doesnt_deadlock_on_full_channel() {
    // Verify sweep handles channel backpressure
}
```

**Actual:** No such test exists.

---

## Recommendations

### Immediate Fixes Required

1. **Add persistent queue for throttled orphans:**
   ```rust
   pub struct RecoveryQueue {
       throttle: RecoveryThrottle,
       pending_orphans: Mutex<Vec<OrphanProcess>>,  // Persist throttled orphans
   }
   ```

2. **Fix time overflow:**
   ```rust
   fn refill(&mut self) {
       if self.refill_period.is_zero() {
           panic!("refill_period cannot be zero");
       }
       // ... rest of logic
   }
   ```

3. **Add backpressure handling:**
   ```rust
   pub async fn run(self, tx: mpsc::Sender<OrphanProcess>) {
       loop {
           interval.tick().await;
           match self.query.query_orphans().await {
               Ok(orphans) => {
                   for orphan in orphans {
                       match tokio::time::timeout(Duration::from_secs(1), tx.send(orphan)).await {
                           Ok(Ok(())) => {},
                           Ok(Err(_)) => return,
                           Err(_) => {
                               // Channel full, backpressure: retry or queue locally
                               self.local_queue.push(orphan);
                           }
                       }
                   }
               }
               Err(e) => tracing::error!("Orphan query failed: {}", e),  // Log errors!
           }
       }
   }
   ```

4. **Add observability:**
   - Track throttle saturation rate
   - Track orphan drop rate
   - Emit metrics for starvation detection

---

## Conclusion

**STATUS: REJECTED**

The implementation fails to satisfy the critical requirements:
1. ✅ "THE SYSTEM SHALL sweep for orphan processes periodically" - Implemented
2. ❌ "WHEN orphan is detected, THE SYSTEM SHALL queue it for recovery via throttled queue" - Orphans can be lost when throttle is full
3. ❌ "IF recovery queue is full, THE SYSTEM SHALL NOT enqueue more orphans" - Violates bounded queue invariant (orphan loss)

**Starvation freedom is not guaranteed.** The implementation must be redesigned to:
1. Persist throttled orphans to prevent data loss
2. Handle channel backpressure without deadlock
3. Add time overflow protection
4. Log errors instead of silent suppression

---

**Reviewer:** Black-Hat Skill  
**Timestamp:** 2026-04-15
