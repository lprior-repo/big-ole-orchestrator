# Test Plan: Atomic Timer Exhaustive Test Strategy

**Bead ID:** ve-jjyka
**Parent:** ve-0dd02 (storage: Implement atomic timer persistence and resumption path)
**Type:** Test Plan (comprehensive coverage across Testing Trophy layers)

---

## Summary

Exhaustive test strategy for the atomic timer subsystem spanning four crates:
`vo-storage/timer_index.rs`, `vo-actor/timer_supervisor.rs`, `vo-actor/timer_lifecycle.rs`,
`vo-actor/timers.rs`, and `vo-core/db_writer_message.rs` (TimerOp variants).

**Scope:**
- Timer persistence (set/delete/scan operations)
- Timer resumption (crash recovery, dual-clock verification)
- Timer cancellation (instance completion cleanup)
- Crash safety (delete-before-dispatch atomicity)

---

## Existing Test Coverage

### vo-storage/timer_index.rs (unit tests)
- 25+ unit tests: TimerKey encoding, TimerValue validation, TimerRecord dual-clock invariant
- timer_set validation (fire_at <= now, zero duration, dual-clock violation, storage failure)
- scan_due_timers (boundary, instance filter, corrupt key/value, storage failure)
- timer_delete (remove, absent key, storage failure)
- scan_all_timers_for_instance (empty, includes future, filters by instance, storage failure)
- 1 proptest: TimerKey lexicographic ordering

### vo-storage/tests/timer_index_red_queen.rs (adversarial)
- 12 attack vectors, 35+ test cases
- Key encoding attacks, TimerValue validation, dual-clock invariant violations
- timer_set edge cases, scan_due_timers boundaries, delete edge cases
- Multiple timers, crash recovery, timer cancellation on completion
- Covers: corrupt keys, saturating arithmetic, future timers, idempotent delete

### vo-actor/timer_supervisor.rs (unit tests)
- verify_dual_clock (5 cases: both met, only wall, only mono, neither, boundary)
- is_overdue (3 cases: overdue, within, boundary)
- validate_timer_record not unit-tested directly

### vo-actor/timer_lifecycle.rs (async unit tests)
- cancel_timers_for_instance (cancels all, returns zero when empty)
- scan_instance_timers (filters by instance)
- has_pending_timers (true/false)
- validate_timer_for_cancellation (accepts matching, rejects different)

### vo-actor/timers.rs (unit tests)
- TimerWaitKey (parse valid/empty/too long/max, for_timer, new_unchecked, ordering, equality, hash)
- SleepState (new valid/rejects zero fire/rejects zero scheduled/rejects fire before scheduled, remaining_ms, is_expired, equality)
- Pure calculations (validate_sleep_duration, compute_fire_at, is_timer_expired, create_sleep_state)
- Error display, proptest invariants (compute_fire_at never wraps, validate_sleep_duration positive_only, sleep_state_remaining never negative)

### vo-core/db_writer_message.rs (TimerOp tests)
- Serde snake_case tag serialization for UpsertTimer/DeleteTimer
- Serde round-trip for both variants
- TimerOp PartialEq (different variants compare unequal)
- AtomicTransition with TimerOps round-trip

### Criterion Benchmarks (vo-storage/benches/timer_index_bench.rs)
- timer_set (100/1k/10k inserts), scan_due_timers, timer_delete
- Mixed workload (80/20 read/write), concurrent inserts, key encoding, range scan

---

## Testing Trophy Allocation

### Layer 1: Unit Tests (Given-When-Then)
**Status:** Mostly Complete -- gaps identified below

**Existing coverage is strong.** The following scenarios are MISSING or WEAK:

#### T1-PERSIST: Timer Persistence Edge Cases

| ID | Given | When | Then | Status |
|----|-------|------|------|--------|
| T1-P01 | Empty storage | `timer_set` with valid args | Timer persisted in BTreeMap | DONE |
| T1-P02 | Timer exists with key K | `timer_set` with same key K, different duration | Timer overwritten (idempotent upsert) | DONE |
| T1-P03 | `fire_at_ms == now_ms` | `timer_set` | Returns `InvalidArgument` | DONE |
| T1-P04 | `fire_at_ms < now_ms` | `timer_set` | Returns `InvalidArgument` | DONE |
| T1-P05 | `duration_ms == 0` | `timer_set` | Returns `InvalidArgument` | DONE |
| T1-P06 | Dual-clock: `fire_at != trigger + duration` | `timer_set` | Returns `InvalidArgument` | DONE |
| T1-P07 | Storage.put fails | `timer_set` | Returns `Storage` error | DONE |
| T1-P08 | `fire_at_ms == u64::MAX` | `TimerKey::new` | Succeeds | DONE (RQ-TK02) |
| T1-P09 | `fire_at_ms == 0` | `TimerKey::new` | Succeeds | DONE (RQ-TK02) |

#### T1-SCAN: Timer Scanning Edge Cases

| ID | Given | When | Then | Status |
|----|-------|------|------|--------|
| T1-S01 | Timer at fire_at=1000, now=1000 | `scan_due_timers` | Timer returned (boundary inclusive) | DONE |
| T1-S02 | Timer at fire_at=1001, now=1000 | `scan_due_timers` | Empty result | DONE |
| T1-S03 | Timer for instance A, scan for B | `scan_due_timers` | Empty result | DONE |
| T1-S04 | Corrupt key (39 bytes) in storage | `scan_due_timers` | Skipped, no error | DONE |
| T1-S05 | Corrupt value (7 bytes) in storage | `scan_due_timers` | Skipped, no error | DONE |
| T1-S06 | `scan_all_timers_for_instance` with future timer | Returns future timer | DONE |
| T1-S07 | Large number of timers (10k+) | `scan_due_timers` | All returned correctly | MISSING |
| T1-S08 | `scan_due_timers` with `now_ms = 0` | No timers due | MISSING |
| T1-S09 | `scan_due_timers` with `now_ms = u64::MAX` | All timers returned | MISSING |

#### T1-CANCEL: Timer Cancellation

| ID | Given | When | Then | Status |
|----|-------|------|------|--------|
| T1-C01 | Instance with 3 timers (past, present, future) | `cancel_timers_for_instance` | All 3 deleted | DONE (RQ-TC01) |
| T1-C02 | Instance with no timers | `cancel_timers_for_instance` | Returns 0 | DONE |
| T1-C03 | Delete specific timer | `timer_delete` | Only that timer removed | DONE (RQ-TC02) |
| T1-C04 | Delete non-existent timer | `timer_delete` | Ok (idempotent) | DONE |
| T1-C05 | Timer for instance A, cancel for B | No timers cancelled | DONE |

#### T1-DUALCLOCK: Dual-Clock Verification (timer_supervisor)

| ID | Given | When | Then | Status |
|----|-------|------|------|--------|
| T1-DC01 | Both clocks agree (fire_at <= now, trigger+duration <= now) | `verify_dual_clock` | true | DONE |
| T1-DC02 | Only wall clock met | `verify_dual_clock` | false | DONE |
| T1-DC03 | Only monotonic met | `verify_dual_clock` | false | DONE |
| T1-DC04 | Neither met | `verify_dual_clock` | false | DONE |
| T1-DC05 | `trigger_time_ms = 0, duration_ms = 0` | `verify_dual_clock` | true (0+0 <= now) -- should this be rejected? | DONE |
| T1-DC06 | Saturating add overflow in monotonic check | `verify_dual_clock` | false (saturates to MAX, MAX > now) | MISSING |

#### T1-SUPERVISOR: TimerSupervisor Integration

| ID | Given | When | Then | Status |
|----|-------|------|------|--------|
| T1-SV01 | Zero tick_interval | `TimerSupervisor::new` | Returns `InvalidConfig` | DONE (in impl) |
| T1-SV02 | `spawn` called twice | `spawn` | Returns `AlreadyRunning` | DONE (in impl) |
| T1-SV03 | `validate_timer_record` with `fire_at_ms = 0` | `validate_timer_record` | Returns `CorruptTimer` | DONE (in impl) |
| T1-SV04 | `validate_timer_record` with `trigger_time_ms = 0` | `validate_timer_record` | Returns `CorruptTimer` | DONE (in impl) |
| T1-SV05 | `validate_timer_record` with `fire_at < trigger_time` | `validate_timer_record` | Returns `CorruptTimer` | DONE (in impl) |
| T1-SV06 | `process_cycle` with no due timers | Returns `CycleResult { 0, 0, 0 }` | MISSING |
| T1-SV07 | `process_cycle` with due timer, delete succeeds, enqueue succeeds | Returns `CycleResult { 1, 0, 0 }` | MISSING |
| T1-SV08 | `process_cycle` with due timer, delete fails | Returns error, `dispatch_errors` incremented | MISSING |
| T1-SV09 | `process_cycle` with due timer, enqueue fails | `dispatch_errors` incremented | MISSING |
| T1-SV10 | `process_cycle` with overdue timer | `overdue_timers` incremented | MISSING |

#### T1-DBWRITER: DbWriterMessage Timer Variants

| ID | Given | When | Then | Status |
|----|-------|------|------|--------|
| T1-DW01 | `UpsertTimer` message | Serialize | JSON contains `"upsert_timer"` | DONE |
| T1-DW02 | `DeleteTimer` message | Serialize | JSON contains `"delete_timer"` | DONE |
| T1-DW03 | `UpsertTimer` | Serde round-trip | Equal after deserialize | DONE |
| T1-DW04 | `DeleteTimer` | Serde round-trip | Equal after deserialize | DONE |
| T1-DW05 | `TimerOp::Upsert` vs `TimerOp::Delete` | PartialEq | Not equal | DONE |

---

### Layer 2: BDD (Dan North Given-When-Then)

#### BDD-01: Timer Persist-Resume Lifecycle

```gherkin
Feature: Timer persist-resume across crash recovery

  Scenario: Timer survives process restart
    Given a timer is set for instance I at fire_at=5000 with duration=1000
    And the process crashes at now=3000
    When the process restarts at now=6000
    Then scan_due_timers returns the timer for instance I
    And the timer record contains fire_at=5000, trigger_time=4000, duration=1000

  Scenario: Future timer not falsely recovered
    Given a timer is set for fire_at=50000
    When the process restarts at now=6000
    Then scan_due_timers returns empty
    And scan_all_timers_for_instance returns the future timer

  Scenario: Multiple timers for different instances recovered correctly
    Given timer T1 for instance A at fire_at=2000
    And timer T2 for instance B at fire_at=3000
    When the process restarts at now=5000
    And scan_due_timers is called for instance A
    Then only T1 is returned
    And T2 is not in the results
```

#### BDD-02: Timer Cancellation on Instance Completion

```gherkin
Feature: All timers cancelled when instance reaches terminal state

  Scenario: Completed instance has no orphan timers
    Given instance I has timers at fire_at=1000, 2000, 5000
    When instance I transitions to Completed
    And cancel_timers_for_instance is called
    Then scan_all_timers_for_instance returns empty
    And no future timer fires for instance I

  Scenario: Cancellation is instance-scoped
    Given instance A has timer T1
    And instance B has timer T2
    When cancel_timers_for_instance is called for instance A
    Then T1 is deleted
    And T2 still exists
```

#### BDD-03: Delete-Before-Dispatch Atomicity

```gherkin
Feature: No double-fire under crash conditions

  Scenario: Timer deleted before dispatch
    Given a due timer exists for instance I
    When timer_delete_before_dispatch is called
    Then the timer is removed from storage
    And the dispatch proceeds

  Scenario: Delete failure prevents dispatch
    Given a due timer exists
    And the storage delete operation fails
    When timer_delete_before_dispatch is called
    Then an error is returned
    And no dispatch occurs
    And the timer remains in storage for retry
```

#### BDD-04: Dual-Clock Drift Protection

```gherkin
Feature: Dual-clock prevents premature or delayed firing

  Scenario: Wall clock says fire but monotonic disagrees
    Given fire_at=1000, trigger=800, duration=400, now=1000
    When verify_dual_clock is called
    Then result is false (monotonic: 800+400=1200 > 1000)

  Scenario: Both clocks agree
    Given fire_at=1000, trigger=800, duration=200, now=1000
    When verify_dual_clock is called
    Then result is true
```

---

### Layer 3: Proptest Invariants

#### PI-01: TimerKey Ordering Invariant
**Existing:** 1 proptest in timer_index.rs
**Missing:**
```
FOR ALL (fire_a, fire_b) where fire_a < fire_b:
  TimerKey(fire_a, iid, tid) < TimerKey(fire_b, iid, tid)  // lexicographic by fire_at
```
Expand to cover: different instance_ids and timer_ids at same fire_at.

#### PI-02: timer_set Never Accepts Invalid Inputs
```
FOR ALL (fire_at, trigger_time, duration, now):
  IF fire_at <= now OR duration == 0 OR fire_at != trigger + duration:
    timer_set(...) == Err(InvalidArgument)
```

#### PI-03: scan_due_timers Correctness
```
FOR ALL timers IN storage, instance_id, now_ms:
  result = scan_due_timers(storage, instance_id, now_ms)
  FOR ALL r IN result:
    r.fire_at_ms <= now_ms
    r.instance_id == instance_id
    r.trigger_time_ms == r.fire_at_ms - r.duration_ms  // reconstructed correctly
```

#### PI-04: Dual-Clock Verification Properties
```
FOR ALL (fire_at, trigger, duration, now) in u64:
  verify_dual_clock(fire_at, trigger, duration, now) IMPLIES
    fire_at <= now AND trigger.saturating_add(duration) <= now
```

#### PI-05: SleepState Invariants
```
FOR ALL (fire_at, scheduled_at) where fire_at >= scheduled_at > 0:
  state = SleepState::new(iid, wk, fire_at, scheduled_at)
  state.duration_ms() == fire_at - scheduled_at
  state.remaining_ms(now) <= state.duration_ms()
  state.is_expired(now) == (fire_at <= now)
```

#### PI-06: compute_fire_at Monotonicity
```
FOR ALL (base, dur) where base + dur does not overflow:
  compute_fire_at(base, dur) == base + dur
  compute_fire_at(base, dur) >= base
```

#### PI-07: Timer Cancellation Idempotency
```
FOR ALL (instance_id, timer_id, fire_at):
  Let n = number of timers for instance before cancel
  cancel_timers_for_instance returns n
  cancel_timers_for_instance returns 0  // second call is idempotent
```

#### PI-08: validate_sleep_duration Exhaustive
**Existing:** 1 proptest in timers.rs
**Missing:** Edge cases at i64::MIN, i64::MAX boundaries.

#### PI-09: TimerOp Serde Round-Trip
```
FOR ALL (timer_id, fire_at) in valid domain:
  serde round-trip of TimerOp::Upsert { timer_id, fire_at } preserves equality
  serde round-trip of TimerOp::Delete { timer_id } preserves equality
```

---

### Layer 4: Crash Safety / Fault Injection

#### CRASH-01: Crash Between timer_set and Commit
```
GIVEN a timer_set operation is in-flight
WHEN the process crashes before the storage write completes
THEN on restart, the timer does NOT exist in storage
AND scan_due_timers returns empty for that timer
```

#### CRASH-02: Crash After timer_set Commit, Before Scan
```
GIVEN a timer was successfully persisted with fire_at=5000
WHEN the process crashes and restarts at now=6000
THEN scan_due_timers finds the timer
AND the timer is dispatched exactly once
```

#### CRASH-03: Crash Between Delete and Dispatch
```
GIVEN delete-before-dispatch deletes the timer from storage
WHEN the process crashes after delete but before dispatch
THEN on restart, the timer is GONE from storage (no double-fire)
AND the workflow remains in sleeping state (missed wake-up -- acceptable per INV-2)
```
**Note:** This is the designed trade-off: missed wake-up is preferable to double-fire.

#### CRASH-04: Crash During Batch AtomicTransition
```
GIVEN an AtomicTransition containing UpsertTimer + RecordInstanceStatus
WHEN the process crashes mid-batch
THEN either:
  a) The batch committed atomically -- timer exists AND status recorded
  b) The batch did NOT commit -- timer does NOT exist AND status NOT recorded
AND partial state is never observable
```

#### CRASH-05: Multiple Restarts With Same Timer
```
GIVEN a timer at fire_at=5000
WHEN the process restarts at now=3000, scans (no result), then crashes
AND restarts at now=6000, scans (finds timer)
THEN the timer is dispatched exactly once across all restarts
```

#### CRASH-06: Storage Corruption Resilience
```
GIVEN storage contains a corrupt timer entry (wrong key length)
WHEN scan_due_timers is called
THEN the corrupt entry is silently skipped
AND all valid entries are still returned
```

---

### Layer 5: Integration Tests

#### INT-01: Full Timer Lifecycle (Set -> Scan -> Delete -> Verify Gone)
```
1. timer_set for instance I, timer T at fire_at=now+5000
2. scan_due_timers at now+1000 -> empty
3. scan_due_timers at now+5000 -> [T]
4. timer_delete T
5. scan_due_timers at now+5000 -> empty
6. scan_all_timers_for_instance -> empty
```

#### INT-02: TimerSupervisor process_cycle Full Path
```
1. Set up MockStorage with a due timer
2. Set up MockWorkQueue
3. Call process_cycle
4. Verify: timer deleted from storage, enqueue_resume called, metrics updated
```

#### INT-03: Fjall Storage Integration (Already covered by benchmarks)
The criterion benchmarks use real fjall storage -- verify correctness alongside performance.

#### INT-04: Cross-Instance Timer Isolation
```
1. Set timer T1 for instance A at fire_at=1000
2. Set timer T2 for instance B at fire_at=1000
3. Cancel all timers for instance A
4. Verify T1 gone, T2 still exists
5. scan_due_timers for instance B -> [T2]
```

---

### Layer 6: Mutation Testing Checkpoints

| Target | Mutation | Expected Kill | Test That Kills |
|--------|----------|---------------|-----------------|
| `timer_set` | Remove `fire_at <= now` check | Test rejects past fire time | fn_timer_set_rejects_fire_at_ms_equal_to_now_ms |
| `timer_set` | Remove dual-clock check | Test rejects mismatch | fn_timer_set_rejects_when_dual_clock_invariant_is_broken |
| `scan_due_timers` | Remove instance filter | Test verifies isolation | fn_scan_due_timers_filters_out_different_instance_id |
| `timer_delete` | Return error on missing key | Test verifies idempotent | fn_timer_delete_returns_ok_when_key_is_absent |
| `verify_dual_clock` | Change AND to OR | Test rejects single-clock pass | verify_dual_clock_returns_false_when_only_wall_clock_met |
| `validate_sleep_duration` | Accept zero | Test rejects zero | validate_sleep_duration_rejects_zero |
| `compute_fire_at` | Remove overflow check | Test catches overflow | compute_fire_at_overflow |
| `cancel_timers_for_instance` | Return wrong count | Test verifies exact count | cancel_timers_for_instance_cancels_all_timers |

---

### Layer 7: Kani Verification Targets

| ID | Property | Target |
|----|----------|--------|
| K-T01 | `TimerKey::new` always returns 40-byte key for valid inputs | timer_index.rs |
| K-T02 | `timer_set` never stores when `fire_at <= now` | timer_index.rs |
| K-T03 | `scan_due_timers` never returns timer with `fire_at > now_ms` | timer_index.rs |
| K-T04 | `verify_dual_clock(a,b,c,d)` implies `a <= d` | timer_supervisor.rs |
| K-T05 | `SleepState::new` rejects when `fire_at < scheduled_at` | timers.rs |
| K-T06 | `compute_fire_at` never wraps on valid (non-overflow) inputs | timers.rs |
| K-T07 | `validate_timer_record` rejects all corrupt states | timer_supervisor.rs |

---

### Layer 8: Cargo-Fuzz Targets

| ID | Target | Input Strategy |
|----|--------|---------------|
| FZ-T01 | `TimerKey::new` | Random 8+16+16 byte arrays |
| FZ-T02 | `timer_set` | Random (fire_at, trigger, duration, now) tuples |
| FZ-T03 | `scan_due_timers` | Pre-populated BTreeMap with random keys, random instance_id |
| FZ-T04 | `TimerOp` serde | Random JSON bytes fed to serde |
| FZ-T05 | `verify_dual_clock` | Random (u64, u64, u64, u64) tuples |
| FZ-T06 | `SleepState::new` | Random (fire_at, scheduled_at) pairs |

---

## Coverage Gap Summary

### HIGH PRIORITY (Missing, should add)

1. **process_cycle integration tests** (T1-SV06 through T1-SV10) -- TimerSupervisor's main logic path is untested beyond pure calc functions
2. **scan_due_timers at boundary now_ms values** (T1-S08, T1-S09) -- zero and MAX
3. **Proptest for scan_due_timers correctness** (PI-03) -- the most critical invariant
4. **Crash safety tests for delete-before-dispatch** (CRASH-03) -- the core atomicity guarantee
5. **Dual-clock saturating overflow** (T1-DC06) -- edge case with u64::MAX trigger/duration

### MEDIUM PRIORITY

6. **TimerOp serde proptest** (PI-09) -- fuzz the serialization
7. **AtomicTransition batch atomicity** (CRASH-04) -- fjall write batch guarantees
8. **validate_sleep_duration at i64 boundaries** (PI-08) -- MIN/MAX edge cases
9. **Large-scale scan tests** (T1-S07) -- performance under load
10. **TimerKey ordering with different instances** (PI-01 expansion)

### LOW PRIORITY (Nice to have)

11. **Kani verification** (K-T01 through K-T07) -- formal proof of invariants
12. **Cargo-fuzz targets** (FZ-T01 through FZ-T06) -- continuous fuzzing
13. **Mutation testing execution** -- run with cargo-mutants

---

## Test Execution Strategy

```bash
# Unit tests for timer_index
cargo test -p vo-storage timer_index

# Unit tests for timer_supervisor
cargo test -p vo-actor timer_supervisor

# Unit tests for timer_lifecycle
cargo test -p vo-actor timer_lifecycle

# Unit tests for timers (wait-key, sleep-state)
cargo test -p vo-actor timers

# Red Queen adversarial suite
cargo test -p vo-storage timer_index_red_queen

# Proptests (requires --features proptest)
cargo test -p vo-storage --features proptest
cargo test -p vo-actor --features proptest

# Benchmarks
cargo bench -p vo-storage timer_index
```
