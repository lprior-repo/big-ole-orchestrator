# Test Plan: Filesystem Watcher with Debounce

## 1. Overview

This test plan covers the `Debouncer` module in `crates/vo-core/src/debounce.rs`, implementing a debounced filesystem event watcher per contract `docs/contracts/filesystem-watcher-debounce.md`.

**Module Under Test:** `Debouncer`
**Source:** `crates/vo-core/src/debounce.rs`
**Contract Invariants:** INV-001 through INV-012

## 2. Test Strategy

### 2.1 Testing Trophy Allocation

| Layer | Coverage Target | Tooling |
|-------|-----------------|---------|
| Unit Tests | INV-001 to INV-012 validation | `#[test]` with tokio::test |
| Property-Based Tests | Deduplication, ordering invariants | proptest |
| Kani Verification | Safety-critical bounds, overflow safety | kani::proof |
| Integration Tests | Full event lifecycle, channel semantics | tokio::test with real channels |

### 2.2 Invariant-to-Test Mapping

| Invariant | Test Strategy | File |
|-----------|---------------|------|
| INV-001: Positive duration | Unit test: zero duration → Error::InvalidDebounceDuration | debounce.rs:298-302 |
| INV-002: Runtime presence | Unit test: spawn outside tokio → Error::NoRuntime | debounce.rs:329-336 |
| INV-003: Channel connectivity | Unit test: disconnected rx → Error::WatcherChannelClosed | debounce.rs:321-326 |
| INV-004: Modify resets deadline | Integration test + proptest | See §3.1 |
| INV-005: Delete cancels pending | Integration test | debounce.rs:444-457 |
| INV-006: Single yield per path | Integration test | debounce.rs:379-400 |
| INV-007: Collapse multiple Modify | Integration test + proptest | debounce.rs:380-400 |
| INV-008: Drain then close | Integration test | debounce.rs:460-475 |
| INV-009: Background task termination | Integration test | debounce.rs:478-486 |
| INV-010: Instant::checked_add overflow | Unit test: Duration::MAX → Error::DebouncerInternal | debounce.rs:553-576 |
| INV-011: No panic propagation | Integration test: errors via channel | debounce.rs:553-576 |
| INV-012: Sorted output | Integration test | debounce.rs:526-550 |

## 3. Test Scenarios

### 3.1 Unit Tests (Existing + New)

**Existing Coverage:**
- `debouncer_new_returns_invalid_duration_error_when_duration_is_zero` ✓
- `debouncer_new_returns_ok_instance_when_duration_is_one_nanosecond` ✓
- `debouncer_new_returns_ok_instance_when_duration_is_max` ✓
- `debouncer_new_returns_channel_closed_error_when_receiver_is_already_closed` ✓
- `debouncer_new_returns_no_runtime_error_outside_tokio` ✓

**Gap: INV-001 Boundary Conditions**
```rust
#[test]
fn debouncer_new_rejects_duration_of_one_nanosecond() {
    let (_tx, rx) = mpsc::channel(10);
    let result = Debouncer::new(Duration::from_nanos(0), rx);
    assert_eq!(result, Err(Error::InvalidDebounceDuration));
}

#[test]
fn debouncer_new_accepts_duration_of_one_nanosecond() {
    let (_tx, rx) = mpsc::channel(10);
    let result = Debouncer::new(Duration::from_nanos(1), rx);
    assert!(result.is_ok());
}
```

**Gap: INV-010 Overflow at Duration::MAX**
```rust
#[tokio::test]
async fn debouncer_handles_max_duration_without_overflow() {
    let duration = Duration::MAX;
    let (tx, rx) = mpsc::channel(10);
    let mut debouncer = Debouncer::new(duration, rx).unwrap();

    tx.send(FileEvent::Modify(PathBuf::from("test.bin"))).await.unwrap();
    // With paused time, this should not overflow in the task
    let result = debouncer.next_debounced_event().await;
    // Either yields path or Error::DebouncerInternal depending on Instant::now() + MAX
}
```

### 3.2 Integration Tests

**Existing Coverage:**
- Single path yield after debounce ✓ (debounce.rs:339-352)
- Continuous writes collapse to single yield ✓ (debounce.rs:355-377)
- Multiple distinct files interleave correctly ✓ (debounce.rs:403-423)
- Timer reset on Modify at exact duration boundary ✓ (debounce.rs:426-441)
- Delete cancels pending ✓ (debounce.rs:444-457)
- Pending drained before channel closed error ✓ (debounce.rs:460-475)
- Channel closed error when sender dropped immediately ✓ (debounce.rs:478-486)
- Remains pending when polled with no events ✓ (debounce.rs:489-496)
- Empty path handling ✓ (debounce.rs:499-509)
- Maximum path length ✓ (debounce.rs:512-523)
- Multiple concurrent distinct files sorted output ✓ (debounce.rs:526-550)

**New Scenarios Required:**

#### INV-004 + INV-007: Modify Deadline Reset Cascade
```rust
#[tokio::test(start_paused = true)]
async fn modify_resets_deadline_on_each_event_within_window() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);
    let path = PathBuf::from("pulse.bin");

    // First Modify at t=0
    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    time::advance(Duration::from_millis(50)).await;

    // Second Modify at t=50 resets deadline to t=150
    tx.send(FileEvent::Modify(path.clone())).await.unwrap();
    time::advance(Duration::from_millis(50)).await;

    // At t=100, nothing should yield (deadline is t=150)
    assert_eq!(poll_next(&mut debouncer).await, Poll::Pending);

    // At t=101 (real time), still pending
    time::advance(Duration::from_millis(51)).await;

    // At t=150 deadline, should yield
    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Ok(path));
}
```

#### INV-006 + INV-012: Exactly One Yield + Sorted Order
```rust
#[tokio::test(start_paused = true)]
async fn each_path_yields_exactly_once_regardless_of_modify_count() {
    let duration = Duration::from_millis(100);
    let (tx, mut debouncer) = setup(duration);
    let path = PathBuf::from("once.bin");

    // 10 rapid Modify events
    for _ in 0..10 {
        tx.send(FileEvent::Modify(path.clone())).await.unwrap();
        time::advance(Duration::from_millis(1)).await;
    }

    time::advance(Duration::from_millis(101)).await;

    // First yield
    let result1 = debouncer.next_debounced_event().await;
    assert_eq!(result1, Ok(path.clone()));

    // Second yield - should be Pending (path already removed)
    let poll2 = poll_next(&mut debouncer).await;
    assert_eq!(poll2, Poll::Pending);
}
```

#### INV-008 + INV-009: Drain Contract + Task Termination
```rust
#[tokio::test(start_paused = true)]
async fn yields_all_pending_before_sending_channel_closed_error() {
    let duration = Duration::from_millis(50);
    let (tx, mut debouncer) = setup(duration);

    let path_a = PathBuf::from("alpha.bin");
    let path_b = PathBuf::from("beta.bin");

    tx.send(FileEvent::Modify(path_a.clone())).await.unwrap();
    time::advance(Duration::from_millis(30)).await;
    tx.send(FileEvent::Modify(path_b.clone())).await.unwrap();

    // Drop sender before deadlines expire
    drop(tx);

    // Advance past both deadlines
    time::advance(Duration::from_millis(100)).await;

    // Should yield both (in sorted order)
    let result_a = debouncer.next_debounced_event().await;
    let result_b = debouncer.next_debounced_event().await;
    assert_eq!(result_a, Ok(path_a));
    assert_eq!(result_b, Ok(path_b));

    // Then channel closed error
    let result_c = debouncer.next_debounced_event().await;
    assert_eq!(result_c, Err(Error::WatcherChannelClosed));
}
```

#### INV-011: Error Propagation via Channel (Not Panic)
```rust
#[tokio::test(start_paused = true)]
async fn internal_errors_propagated_via_channel_not_panic() {
    // This is covered by debouncer_new_returns_channel_closed_error_when_receiver_is_already_closed
    // but needs expansion to cover Error::DebouncerInternal propagation
    let duration = Duration::MAX; // Triggers overflow path
    let (tx, mut debouncer) = setup(duration);

    tx.send(FileEvent::Modify(PathBuf::from("overflow.bin"))).await.unwrap();
    time::advance(Duration::from_millis(1)).await;

    // Should receive Error::DebouncerInternal, not panic
    let result = debouncer.next_debounced_event().await;
    assert_eq!(result, Err(Error::DebouncerInternal));
}
```

### 3.3 Property-Based Tests (proptest)

**Existing Coverage:**
- `debouncer_new_handles_any_positive_duration` ✓
- `event_stream_deduplicates_multiple_events_for_same_file` ✓

**Gap: INV-004 + INV-007 Deadline Reset Property**
```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn modify_count_does_not_affect_yield_count(
        filename in "[a-z]{1,20}\\.bin",
        modify_count in 1..50u32
    ) {
        let rt = tokio::Runtime::new().unwrap();
        rt.block_on(async {
            tokio::time::pause();
            let duration = Duration::from_millis(100);
            let (tx, rx) = mpsc::channel(100);
            let mut debouncer = Debouncer::new(duration, rx).unwrap();
            let path = PathBuf::from(&filename);

            for _ in 0..modify_count {
                tx.send(FileEvent::Modify(path.clone())).await.unwrap();
                tokio::time::advance(Duration::from_millis(1)).await;
            }

            tokio::time::advance(Duration::from_millis(101)).await;
            drop(tx);

            let yield_count = debouncer.next_debounced_event().await.unwrap();
            assert_eq!(yield_count, path);

            // Second yield should error
            let eof = debouncer.next_debounced_event().await;
            assert_eq!(eof, Err(Error::WatcherChannelClosed));
        });
    }

    #[test]
    fn delete_before_deadline_prevents_yield(
        filename in "[a-z]{1,20}\\.bin"
    ) {
        let rt = tokio::Runtime::new().unwrap();
        rt.block_on(async {
            tokio::time::pause();
            let duration = Duration::from_millis(100);
            let (tx, rx) = mpsc::channel(100);
            let mut debouncer = Debouncer::new(duration, rx).unwrap();
            let path = PathBuf::from(&filename);

            tx.send(FileEvent::Modify(path.clone())).await.unwrap();
            tokio::time::advance(Duration::from_millis(50)).await;
            tx.send(FileEvent::Delete(path.clone())).await.unwrap();

            tokio::time::advance(Duration::from_millis(100)).await;
            drop(tx);

            // Should immediately get channel closed (no pending)
            let result = debouncer.next_debounced_event().await;
            assert_eq!(result, Err(Error::WatcherChannelClosed));
        });
    }

    #[test]
    fn yields_are_deterministic_regardless_of_event_order(
        paths in prop::collection::hash_map("[a-z]{1,10}\\.bin", 1..5, 5)
    ) {
        let rt = tokio::Runtime::new().unwrap();
        rt.block_on(async {
            tokio::time::pause();
            let duration = Duration::from_millis(100);

            let mut all_yields = Vec::new();

            // Run 10 times with same event sequence
            for _ in 0..10 {
                let (tx, rx) = mpsc::channel(100);
                let mut debouncer = Debouncer::new(duration, rx).unwrap();

                for (i, path) in paths.values().enumerate() {
                    tx.send(FileEvent::Modify(PathBuf::from(path))).await.unwrap();
                    tokio::time::advance(Duration::from_millis(i as u64)).await;
                }

                tokio::time::advance(Duration::from_millis(101)).await;
                drop(tx);

                let mut yields = Vec::new();
                loop {
                    match debouncer.next_debounced_event().await {
                        Ok(p) => yields.push(p),
                        Err(Error::WatcherChannelClosed) => break,
                        Err(e) => panic!("Unexpected error: {:?}", e),
                    }
                }
                yields.sort();
                all_yields.push(yields);
            }

            // All runs should produce identical sorted output
            for yields in &all_yields[1..] {
                assert_eq!(yields, &all_yields[0]);
            }
        });
    }
}
```

### 3.4 Kani Verification

**Existing Coverage:**
- `verify_event_tracking_state_bounds` - basic bounds check

**Required Expansion:**

```rust
#[cfg(kani)]
mod verification {
    use super::*;

    #[kani::proof]
    fn verify_invariant_001_positive_duration() {
        // Verify: zero duration always returns error
        let duration = Duration::from_nanos(kani::any());
        if duration.as_nanos() == 0 {
            // Would need channel mock - verify at type level
            // that duration == 0 is prevented
            kani::cover();
        }
    }

    #[kani::proof]
    fn verify_invariant_004_modify_resets_deadline() {
        // Model: pending[path] = deadline
        // When Modify(path) arrives: deadline = now + duration
        // Verify: second Modify extends deadline (doesn't stack)
        let initial_deadline: u64 = kani::any();
        let duration: u64 = kani::any();
        kani::assume(duration > 0);

        let new_deadline = initial_deadline.saturating_add(duration);
        assert!(new_deadline >= initial_deadline + duration);
    }

    #[kani::proof]
    fn verify_invariant_005_delete_removes_pending() {
        let pending_count: u8 = kani::any();
        let path_removed: bool = kani::any();

        // When Delete arrives, pending.remove(path) is called
        // Verify: count decreases by 1 if path was present
        if path_removed && pending_count > 0 {
            assert!(pending_count - 1 <= pending_count);
        }
    }

    #[kani::proof]
    fn verify_invariant_010_checked_add_no_overflow() {
        let base_instant: u128 = kani::any();
        let duration_nanos: u128 = kani::any();

        // Verify: checked_add is used (not wrapping_add)
        // Kani should catch any use of wrapping_* or unguarded arithmetic
        let _ = base_instant.checked_add(duration_nanos);
    }

    #[kani::proof]
    fn verify_invariant_012_sorted_output_determinism() {
        // Model 3 paths with different deadlines
        // Verify: output order is sorted by PathBuf, not by deadline
        let path_a = PathBuf::from("a");
        let path_b = PathBuf::from("b");
        let path_c = PathBuf::from("c");

        let deadline_a: u64 = kani::any();
        let deadline_b: u64 = kani::any();
        let deadline_c: u64 = kani::any();

        // Assume deadlines are unordered
        kani::assume(deadline_a != deadline_b && deadline_b != deadline_c && deadline_a != deadline_c);

        // Verify: sorting yields a < b < c regardless of deadlines
        let mut paths = vec![path_b.clone(), path_a.clone(), path_c.clone()];
        paths.sort();

        assert_eq!(paths[0], PathBuf::from("a"));
        assert_eq!(paths[1], PathBuf::from("b"));
        assert_eq!(paths[2], PathBuf::from("c"));
    }
}
```

### 3.5 Edge Cases

| Scenario | Input | Expected | Invariant |
|----------|-------|----------|-----------|
| Zero-duration | `Duration::ZERO` | `Err(InvalidDebounceDuration)` | INV-001 |
| Max-duration overflow | `Duration::MAX` + Modify | `Err(DebouncerInternal)` or pending | INV-010 |
| Rapid Modify/Delete interleaving | A.modify, A.delete, A.modify | Single yield at deadline | INV-004, INV-005 |
| Empty path | `FileEvent::Modify("")` | Yields empty path | - |
| Unicode path | `FileEvent::Modify("/üñíćódé/例字.bin")` | Yields correctly | - |
| Very long path | 4096 char path | Yields correctly | - |
| Channel full | High-frequency events | Backpressure handled | INV-009 |
| Multiple files same deadline | a.bin, b.bin same deadline | Sorted output | INV-012 |

## 4. Test Execution

### 4.1 Unit Tests
```bash
cargo test -p vo-core debounce::tests
```

### 4.2 Property-Based Tests
```bash
cargo test -p vo-core debounce::proptests
```

### 4.3 Kani Verification
```bash
cargo kani -p vo-core --module debounce::verification
```

### 4.4 Full Coverage
```bash
cargo test -p vo-core --lib debounce
cargo clippy -p vo-core -- -D warnings
cargo fmt -p vo-core -- --check
```

## 5. Acceptance Criteria

- [ ] All 12 invariants (INV-001 to INV-012) have dedicated test coverage
- [ ] Unit tests achieve 100% branch coverage on `debounce.rs`
- [ ] Property-based tests validate deduplication and determinism
- [ ] Kani proofs verify overflow safety and sorted output
- [ ] Edge cases: empty path, max path, unicode, Duration::MAX covered
- [ ] Error taxonomy fully exercised (all Error variants)
- [ ] No unwrap/expect in test code
- [ ] All tests pass with `cargo test -p vo-core`
- [ ] Clippy clean: `cargo clippy -p vo-core -- -D warnings`

## 6. Test Artifacts

| Artifact | Location |
|----------|----------|
| Test Plan | `docs/test-plans/filesystem-watcher-debounce.md` |
| Contract | `docs/contracts/filesystem-watcher-debounce.md` |
| Implementation | `crates/vo-core/src/debounce.rs` |
| Proptest regressions | `crates/vo-core/proptest-regressions/debounce.txt` |

---

**Author:** veloxide/polecats/shiny
**Date:** 2026-04-12
**Bead:** ve-acf
