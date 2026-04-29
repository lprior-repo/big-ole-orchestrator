# Test Plan: Async Process Supervisor (ve-mbe8)

## Summary
- Behaviors identified: 52
- Trophy allocation: 38 unit / 22 integration / 4 e2e / 12 proptest (Total 76 tests)
- Proptest invariants: 12
- Fuzz targets: 2
- Kani harnesses: 2
- Target Mutation Kill Rate: ≥90%

## 1. Behavior Inventory

### SpawnRecord Construction & Transitions (8)
1. `SpawnRecord::new` creates a record in `Spawn` phase with `spawn_attempts = 1` and `last_error = None`.
2. `SpawnRecord::new` preserves the provided `instance_id`, `command`, and `spawn_id`.
3. `transition_to_health_check` sets phase to `HealthCheck`, preserves all other fields.
4. `transition_to_running` sets phase to `Running`, preserves all other fields.
5. `transition_to_shutdown` sets phase to `Shutdown`, preserves all other fields.
6. `respawn` creates a new record with phase `Spawn`, `spawn_attempts` incremented by 1, `health_checks = 0`, `last_error = None`.
7. `respawn` uses `saturating_add` for `spawn_attempts` (no overflow on `u32::MAX`).
8. `respawn` accepts `None` for `new_spawn_id` (spawn_id becomes `None`).

### SpawnPhase (4)
9. `SpawnPhase` variants are `Spawn`, `HealthCheck`, `Running`, `Shutdown`, `Terminated`, `Failed`.
10. `SpawnPhase::Display` formats as lowercase: `"spawn"`, `"health-check"`, `"running"`, `"shutdown"`, `"terminated"`, `"failed"`.
11. `SpawnPhase` derives `Copy`, `Clone`, `PartialEq`, `Eq`, `Hash`.
12. No invalid transitions exist at the type level — transitions are validated by behavior tests.

### SpawnSupervisorError Classification (8)
13. `is_transient` returns `true` for `StorageError`, `InstanceNotFound`, `MailboxFull`, `DispatchError`.
14. `is_transient` returns `false` for all non-transient variants (`CorruptSpawn`, `InvalidConfig`, `ZombieDetected`, `AlreadyRunning`, `ShutdownTimeout`, `SpawnFailed`, `HealthCheckFailed`, `ProcessExited`, `NotRunning`, `AlreadyShutdown`, `AtomicityViolation`).
15. `is_fatal` returns `true` for `CorruptSpawn`, `InvalidConfig`, `ZombieDetected`.
16. `is_fatal` returns `false` for all non-fatal variants.
17. `SpawnSupervisorError::Display` produces a non-empty, human-readable string for every variant.
18. `SpawnSupervisorError` implements `std::error::Error`.
19. `SpawnSupervisorError::SpawnFailed` carries `command` and `error` fields.
20. `SpawnSupervisorError::HealthCheckFailed` carries `instance_id`, `check_number`, and `error` fields.

### Counter & Metrics (3)
21. `Counter::new()` starts at 0.
22. `Counter::incr()` increments by 1 atomically.
23. `Counter::get()` returns the current value.

### SpawnSupervisor Construction (6)
24. `SpawnSupervisor::new` rejects `health_check_interval = 0` with `InvalidConfig`.
25. `SpawnSupervisor::new` rejects `max_health_checks = 0` with `InvalidConfig`.
26. `SpawnSupervisor::new` rejects `initial_backoff = 0` with `InvalidConfig`.
27. `SpawnSupervisor::new` rejects `backoff_multiplier < 1.0` with `InvalidConfig`.
28. `SpawnSupervisor::new` accepts valid parameters and returns a `SpawnSupervisor`.
29. `SpawnSupervisor::new` initializes metrics to zero defaults.

### Pure Functions (6)
30. `calculate_backoff_delay(initial, 2.0, 1)` returns `initial` (no multiplier on first attempt).
31. `calculate_backoff_delay` applies exponential growth: attempt 2 = `initial * 2`, attempt 3 = `initial * 4` (with multiplier 2.0).
32. `calculate_backoff_delay` with multiplier `1.0` returns `initial` for all attempts (constant backoff).
33. `calculate_backoff_delay` saturates on overflow (attempt near `u32::MAX`).
34. `is_zombie_state` returns `true` when phase is `Failed` AND `spawn_attempts > 3`.
35. `is_zombie_state` returns `false` for non-`Failed` phase OR `spawn_attempts <= 3`.

36. `should_respawn` returns `true` when phase is `Failed` AND `spawn_attempts < max_attempts`.
37. `should_respawn` returns `false` when phase is not `Failed`.
38. `should_respawn` returns `false` when `spawn_attempts >= max_attempts`.

### Supervisor Lifecycle (6)
39. `spawn()` transitions state from `Stopped` to `Running`.
40. `spawn()` returns a `SpawnSupervisorHandle` with `current_state() == Running`.
41. `shutdown()` transitions state: `Running → ShuttingDown → ShutDown`.
42. `shutdown()` waits for the background task to complete before returning.
43. Calling `shutdown()` on an already-shut-down handle returns `AlreadyShutdown`.
44. The run loop exits cleanly on shutdown signal (no panic, no error).

### Process Cycle (6)
45. `process_cycle` scans for records in `Spawn` phase and attempts to spawn them.
46. `process_cycle` skips records where `spawn_attempts > max_spawn_attempts`.
47. `process_cycle` transitions successful spawns through `Spawn → HealthCheck → Running`.
48. `process_cycle` records `last_error` on spawn failure.
49. `process_cycle` increments `spawns_failed` metric on failure.
50. `process_cycle` returns `CycleResult` with correct counts.

### Health Check Logic (4)
51. `perform_health_checks` passes when all checks return `Ok(true)`.
52. `perform_health_checks` fails after `max_health_checks` unsuccessful checks.
53. `perform_health_checks` returns `HealthCheckFailed` on `Err` from `check_health`.
54. `perform_health_checks` sleeps between checks (respects `health_check_interval` contract).

### Cancellation & Concurrency (2)
55. Shutdown during an in-flight `process_cycle` completes the current cycle before exiting.
56. The `state_sender` watch channel correctly broadcasts all state transitions.

## 2. Trophy Allocation

*   **Unit Tests (38)**: Cover all pure functions (`calculate_backoff_delay`, `is_zombie_state`, `should_respawn`), `SpawnRecord` construction and transitions, `SpawnPhase` display, `SpawnSupervisorError` classification and display, `Counter` operations, `SpawnSupervisor::new` validation, and `ProcessHandle::new`.
*   **Integration Tests (22)**: Cover `process_cycle` with mock `SpawnStorage`/`ProcessManager`/`WorkQueue`, supervisor lifecycle (`spawn` → `shutdown`), health check sequences, error propagation through the cycle, metrics increment paths, and shutdown cancellation safety.
*   **E2E Tests (4)**: Full lifecycle test spawning a real supervisor with mock dependencies, verifying state transitions end-to-end.
*   **Proptest (12)**: Property-based testing for backoff calculation monotonicity, state machine invariant preservation, error classification completeness, and arbitrary `SpawnRecord` field combinations.

## 3. BDD Scenarios

### Behavior: SpawnRecord Construction
Given: An `InstanceId` and command string `"./worker"`
When: `SpawnRecord::new` is called
Then: Record has `phase = Spawn`, `spawn_attempts = 1`, `health_checks = 0`, `last_error = None`

### Behavior: SpawnRecord Transition to HealthCheck
Given: A `SpawnRecord` in `Spawn` phase
When: `transition_to_health_check()` is called
Then: Phase is `HealthCheck`, all other fields unchanged

### Behavior: SpawnRecord Transition to Running
Given: A `SpawnRecord` in `HealthCheck` phase
When: `transition_to_running()` is called
Then: Phase is `Running`, all other fields unchanged

### Behavior: SpawnRecord Transition to Shutdown
Given: A `SpawnRecord` in `Running` phase
When: `transition_to_shutdown()` is called
Then: Phase is `Shutdown`, all other fields unchanged

### Behavior: SpawnRecord Respawn
Given: A `SpawnRecord` with `spawn_attempts = 3`, `phase = Failed`
When: `respawn(Some(new_id))` is called
Then: New record has `phase = Spawn`, `spawn_attempts = 4`, `health_checks = 0`, `last_error = None`, `spawn_id = Some(new_id)`

### Behavior: SpawnRecord Respawn Overflow Protection
Given: A `SpawnRecord` with `spawn_attempts = u32::MAX`
When: `respawn(None)` is called
Then: `spawn_attempts` remains `u32::MAX` (saturating_add)

### Behavior: Backoff Calculation First Attempt
Given: `initial_backoff_ms = 1000`, `multiplier = 2.0`, `attempt = 1`
When: `calculate_backoff_delay` is called
Then: Returns `1000`

### Behavior: Backoff Calculation Exponential Growth
Given: `initial_backoff_ms = 1000`, `multiplier = 2.0`, `attempt = 3`
When: `calculate_backoff_delay` is called
Then: Returns `4000`

### Behavior: Backoff Calculation Constant (multiplier = 1.0)
Given: `initial_backoff_ms = 1000`, `multiplier = 1.0`, `attempt = 10`
When: `calculate_backoff_delay` is called
Then: Returns `1000`

### Behavior: Zombie State Detection (True)
Given: A `SpawnRecord` with `phase = Failed`, `spawn_attempts = 5`
When: `is_zombie_state` is called
Then: Returns `true`

### Behavior: Zombie State Detection (False — not failed)
Given: A `SpawnRecord` with `phase = Running`, `spawn_attempts = 5`
When: `is_zombie_state` is called
Then: Returns `false`

### Behavior: Zombie State Detection (False — low attempts)
Given: A `SpawnRecord` with `phase = Failed`, `spawn_attempts = 3`
When: `is_zombie_state` is called
Then: Returns `false`

### Behavior: Should Respawn (True)
Given: A `SpawnRecord` with `phase = Failed`, `spawn_attempts = 2`, `max_attempts = 5`
When: `should_respawn` is called
Then: Returns `true`

### Behavior: Should Respawn (False — at limit)
Given: A `SpawnRecord` with `phase = Failed`, `spawn_attempts = 5`, `max_attempts = 5`
When: `should_respawn` is called
Then: Returns `false`

### Behavior: Should Respawn (False — not failed)
Given: A `SpawnRecord` with `phase = Running`, `spawn_attempts = 2`, `max_attempts = 5`
When: `should_respawn` is called
Then: Returns `false`

### Behavior: Supervisor Rejects Zero Health Check Interval
Given: `health_check_interval = Duration::ZERO`
When: `SpawnSupervisor::new` is called with valid other params
Then: Returns `Err(InvalidConfig("health_check_interval must be > 0"))`

### Behavior: Supervisor Rejects Zero Max Health Checks
Given: `max_health_checks = 0`
When: `SpawnSupervisor::new` is called with valid other params
Then: Returns `Err(InvalidConfig("max_health_checks must be > 0"))`

### Behavior: Supervisor Rejects Zero Initial Backoff
Given: `initial_backoff = Duration::ZERO`
When: `SpawnSupervisor::new` is called with valid other params
Then: Returns `Err(InvalidConfig("initial_backoff must be > 0"))`

### Behavior: Supervisor Rejects Low Backoff Multiplier
Given: `backoff_multiplier = 0.5`
When: `SpawnSupervisor::new` is called with valid other params
Then: Returns `Err(InvalidConfig("backoff_multiplier must be >= 1.0"))`

### Behavior: Supervisor Accepts Valid Config
Given: All valid parameters
When: `SpawnSupervisor::new` is called
Then: Returns `Ok(SpawnSupervisor)` with correct field values

### Behavior: Error Transient Classification
Given: `SpawnSupervisorError::StorageError("db down")`
When: `is_transient()` is called
Then: Returns `true`

### Behavior: Error Fatal Classification
Given: `SpawnSupervisorError::CorruptSpawn("bad data")`
When: `is_fatal()` is called
Then: Returns `true`

### Behavior: Error Display — SpawnFailed
Given: `SpawnSupervisorError::SpawnFailed { command: "./worker", error: "ENOENT" }`
When: `to_string()` is called
Then: Returns `"Spawn failed for './worker': ENOENT"`

### Behavior: Error Display — HealthCheckFailed
Given: `SpawnSupervisorError::HealthCheckFailed { instance_id, check_number: 3, error: "timeout" }`
When: `to_string()` is called
Then: Contains `"Health check 3 failed"` and the instance ID

### Behavior: Error Display — ZombieDetected
Given: `SpawnSupervisorError::ZombieDetected { instance_id, pid: 1234 }`
When: `to_string()` is called
Then: Contains `"Zombie detected"` and `"pid=1234"`

### Behavior: Error Display — ProcessExited
Given: `SpawnSupervisorError::ProcessExited { instance_id, pid: 5678, exit_code: 1 }`
When: `to_string()` is called
Then: Contains `"Process exited"` and `"code=1"`

### Behavior: Error Display — ShutdownTimeout
Given: `SpawnSupervisorError::ShutdownTimeout(Duration::from_secs(30))`
When: `to_string()` is called
Then: Contains `"Shutdown timeout"`

### Behavior: Counter Starts at Zero
Given: A new `Counter`
When: `get()` is called
Then: Returns `0`

### Behavior: Counter Increments
Given: A `Counter` at `0`
When: `incr()` is called 3 times
Then: `get()` returns `3`

### Behavior: ProcessHandle Construction
Given: PID `1234` and command `"./worker"`
When: `ProcessHandle::new(1234, "./worker".to_string())` is called
Then: `handle.pid == 1234` and `handle.command == "./worker"`

### Behavior: Supervisor Spawn — State Transition
Given: A valid `SpawnSupervisor` with mock dependencies
When: `spawn()` is called
Then: Handle's `current_state()` is `Running`

### Behavior: Supervisor Shutdown — Clean
Given: A running `SpawnSupervisorHandle`
When: `shutdown()` is called
Then: Returns `Ok(())`, state reaches `ShutDown`

### Behavior: Supervisor Shutdown — Already Shutdown
Given: An already-shut-down `SpawnSupervisorHandle`
When: `shutdown()` is called again (after first shutdown consumed self)
Then: This is a type-level impossibility (shutdown takes `self`), verified by compilation

### Behavior: Process Cycle — Successful Spawn
Given: Storage returns a `Spawn` phase record, `ProcessManager` succeeds, health checks pass
When: `process_cycle()` is called
Then: Record transitions `Spawn → HealthCheck → Running`, metrics: `spawns_successful = 1`

### Behavior: Process Cycle — Spawn Failure
Given: Storage returns a `Spawn` phase record, `ProcessManager` returns `SpawnFailed`
When: `process_cycle()` is called
Then: Record gets `last_error = Some(SpawnFailed)`, metrics: `spawns_failed = 1`

### Behavior: Process Cycle — Max Attempts Exceeded
Given: Storage returns a record with `spawn_attempts > max_spawn_attempts`
When: `process_cycle()` is called
Then: Record is skipped, `spawns_failed` metric incremented

### Behavior: Process Cycle — Health Check Failure
Given: A record transitions to `HealthCheck`, but `check_health` returns `Ok(false)` for all attempts
When: `process_cycle()` is called
Then: `health_checks_failed` metric incremented

### Behavior: Process Cycle — Storage Save Failure
Given: A successful spawn + health check, but `save_spawn_record` fails
When: `process_cycle()` is called
Then: `dispatch_errors` metric incremented, error logged

### Behavior: CycleResult Counts
Given: 3 records processed, 1 health check, 1 error, 1 respawn
When: `process_cycle()` completes
Then: `CycleResult { spawns_processed: 3, health_checks: 1, errors: 1, respawns: 1 }`

### Behavior: Supervisor State Broadcast
Given: A running supervisor
When: State transitions occur (spawn, shutdown)
Then: `watch::Receiver` receives `Running` then `ShuttingDown` then `ShutDown`

### Behavior: Shutdown During In-Flight Cycle
Given: A supervisor in the middle of `process_cycle`
When: `shutdown()` is called
Then: Current cycle completes, then loop exits cleanly

### Behavior: SpawnPhase Display — All Variants
Given: Each `SpawnPhase` variant
When: `to_string()` is called
Then: `Spawn → "spawn"`, `HealthCheck → "health-check"`, `Running → "running"`, `Shutdown → "shutdown"`, `Terminated → "terminated"`, `Failed → "failed"`

## 4. Proptest Invariants

### Proptest: Backoff Monotonicity
Invariant: For any `initial > 0`, `multiplier >= 1.0`, and `attempt_a < attempt_b`: `calculate_backoff_delay(initial, multiplier, attempt_a) <= calculate_backoff_delay(initial, multiplier, attempt_b)`.
Strategy: `initial: 1..=10000`, `multiplier: 1.0..=10.0`, `attempt: 1..=30`.

### Proptest: Backoff Lower Bound
Invariant: `calculate_backoff_delay(initial, multiplier, attempt) >= initial` for all `attempt >= 1` when `multiplier >= 1.0`.
Strategy: Same as above.

### Proptest: Backoff Overflow Safety
Invariant: `calculate_backoff_delay(initial, multiplier, attempt)` never panics for any `initial: u64`, `multiplier: f64`, `attempt: u32`.
Strategy: Arbitrary `u64`, `f64` (filtered to finite), `u32`.

### Proptest: SpawnRecord Transition Immutability
Invariant: For any `SpawnRecord`, calling any transition method preserves all fields except `spawn_phase`.
Strategy: Arbitrary `SpawnRecord` fields.

### Proptest: Error Classification Disjointness
Invariant: No error variant is both `is_transient() == true` AND `is_fatal() == true`.
Strategy: Exhaustive enumeration of all variants.

### Proptest: Error Display Non-Empty
Invariant: Every `SpawnSupervisorError` variant produces a non-empty string from `to_string()`.
Strategy: Exhaustive enumeration.

### Proptest: Respawn Attempt Monotonicity
Invariant: For any `SpawnRecord`, `record.respawn(id).spawn_attempts >= record.spawn_attempts`.
Strategy: Arbitrary `SpawnRecord`, arbitrary `Option<SpawnId>`.

### Proptest: Respawn Phase Reset
Invariant: For any `SpawnRecord`, `record.respawn(id).spawn_phase == SpawnPhase::Spawn`.
Strategy: Arbitrary `SpawnRecord`.

### Proptest: Respawn Health Check Reset
Invariant: For any `SpawnRecord`, `record.respawn(id).health_checks == 0`.
Strategy: Arbitrary `SpawnRecord`.

### Proptest: is_zombie_state Consistency
Invariant: `is_zombie_state(r) == true` implies `r.spawn_phase == Failed && r.spawn_attempts > 3`.
Strategy: Arbitrary `SpawnRecord`.

### Proptest: should_respawn Consistency
Invariant: `should_respawn(r, max) == true` implies `r.spawn_phase == Failed && r.spawn_attempts < max`.
Strategy: Arbitrary `SpawnRecord`, `max: 1..=100`.

### Proptest: SpawnRecord New Defaults
Invariant: `SpawnRecord::new(id, cmd, sid)` always has `spawn_attempts == 1` and `health_checks == 0` and `last_error == None` and `spawn_phase == Spawn`.
Strategy: Arbitrary `InstanceId`, `String`, `Option<SpawnId>`.

## 5. Fuzz Targets

### Fuzz Target: Backoff Delay Calculation
Input type: `(u64, f64, u32)` — initial_backoff_ms, multiplier, attempt
Risk: Panic on `f64::NAN` or `f64::INFINITY` in `powf`, integer overflow in duration calculation.
Corpus seeds: `(1000, 2.0, 1)`, `(1, 1000.0, 100)`, `(u64::MAX, 2.0, 100)`, `(1000, f64::NAN, 1)`.

### Fuzz Target: Error Display Formatting
Input type: Constructed `SpawnSupervisorError` with arbitrary string/pid/exit_code fields
Risk: Panic in `write!` macro on extremely long strings or edge cases.
Corpus seeds: Empty strings, very long strings (1MB), strings with format specifiers `{}`, unicode strings.

## 6. Kani Harnesses

### Kani Harness: Backoff Delay Bounds
Property: For all valid inputs, `calculate_backoff_delay` returns a value that is `>= initial_backoff_ms` when `multiplier >= 1.0`, and never panics.
Bound: Depth 5.
Rationale: The backoff calculation involves float math and integer casting — Kani verifies no UB or logic error.

### Kani Harness: SpawnRecord Transition Correctness
Property: After any sequence of transitions (up to length 4), the record satisfies phase atomicity: exactly one phase, no field corruption.
Bound: Depth 4.
Rationale: Verifies the transition chain doesn't corrupt record state.

## 7. Mutation Checkpoints

Critical mutations to survive:
- Changing `spawn_attempts > 3` to `spawn_attempts > 2` in `is_zombie_state` must be caught by the boundary test at `spawn_attempts == 3`.
- Changing `spawn_attempts < max_attempts` to `spawn_attempts <= max_attempts` in `should_respawn` must be caught by the equality boundary test.
- Removing `health_check_interval.is_zero()` check in `SpawnSupervisor::new` must be caught by the zero-interval rejection test.
- Removing `backoff_multiplier < 1.0` check must be caught by the multiplier validation test.
- Changing `saturating_add(1)` to `add(1)` in `respawn` must be caught by the `u32::MAX` overflow test.
- Swapping `is_transient` and `is_fatal` match arms must be caught by the classification tests.
- Removing `spawns_successful.incr()` on successful spawn must be caught by the metrics integration test.
- Changing `scan_spawns_by_phase(SpawnPhase::Spawn, 100)` to `SpawnPhase::HealthCheck` must be caught by the process_cycle spawn test.

Threshold: 90% mutation kill rate minimum.
Coverage: 90% line coverage minimum.

## 8. Combinatorial Coverage Matrix

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| SpawnRecord new | Valid params | Phase=Spawn, attempts=1 | Unit |
| Transition Spawn→HealthCheck | Spawn record | Phase=HealthCheck | Unit |
| Transition HealthCheck→Running | HealthCheck record | Phase=Running | Unit |
| Transition Running→Shutdown | Running record | Phase=Shutdown | Unit |
| Respawn (attempts=3) | Failed record | Phase=Spawn, attempts=4 | Unit |
| Respawn (attempts=u32::MAX) | Failed record | Phase=Spawn, attempts=u32::MAX | Unit |
| Backoff attempt=1 | initial=1000, mult=2.0 | 1000 | Unit |
| Backoff attempt=2 | initial=1000, mult=2.0 | 2000 | Unit |
| Backoff attempt=3 | initial=1000, mult=2.0 | 4000 | Unit |
| Backoff multiplier=1.0 | initial=1000, mult=1.0 | 1000 (constant) | Unit |
| is_zombie (Failed, attempts=5) | phase=Failed, attempts=5 | true | Unit |
| is_zombie (Failed, attempts=3) | phase=Failed, attempts=3 | false | Unit |
| is_zombie (Running, attempts=5) | phase=Running, attempts=5 | false | Unit |
| should_respawn (Failed, 2/5) | phase=Failed, attempts=2 | true | Unit |
| should_respawn (Failed, 5/5) | phase=Failed, attempts=5 | false | Unit |
| should_respawn (Running, 2/5) | phase=Running, attempts=2 | false | Unit |
| Supervisor new: zero interval | interval=0 | Err(InvalidConfig) | Unit |
| Supervisor new: zero max_checks | max_checks=0 | Err(InvalidConfig) | Unit |
| Supervisor new: zero backoff | backoff=0 | Err(InvalidConfig) | Unit |
| Supervisor new: multiplier<1 | multiplier=0.5 | Err(InvalidConfig) | Unit |
| Supervisor new: valid | All valid | Ok(SpawnSupervisor) | Unit |
| Error is_transient: StorageError | StorageError("x") | true | Unit |
| Error is_transient: CorruptSpawn | CorruptSpawn("x") | false | Unit |
| Error is_fatal: CorruptSpawn | CorruptSpawn("x") | true | Unit |
| Error is_fatal: StorageError | StorageError("x") | false | Unit |
| Error Display: all variants | Each variant | Non-empty string | Unit |
| Counter new | Default | 0 | Unit |
| Counter incr×3 | 3 increments | 3 | Unit |
| SpawnPhase Display | Each variant | Correct string | Unit |
| Process Cycle: success | Mock returns success | Running record, metrics | Integration |
| Process Cycle: spawn fail | Mock returns SpawnFailed | last_error set | Integration |
| Process Cycle: max exceeded | attempts > max | Skipped, metric++ | Integration |
| Process Cycle: health fail | check_health→false | HealthChecksFailed++ | Integration |
| Process Cycle: storage fail | save returns err | DispatchErrors++ | Integration |
| Process Cycle: CycleResult | Mixed scenario | Correct counts | Integration |
| Lifecycle: spawn→shutdown | Valid supervisor | Stopped→Running→ShutDown | Integration |
| Lifecycle: state broadcast | Watch receiver | Receives all states | Integration |
| Shutdown during cycle | In-flight process | Cycle completes first | Integration |
| Health check: all pass | check_health→true | Ok(()) | Integration |
| Health check: all fail | check_health→false×N | Err(HealthCheckFailed) | Integration |
| Health check: error on check | check_health→Err | Err(HealthCheckFailed) | Integration |
| Full lifecycle E2E | Real mocks, full cycle | Correct end-to-end state | E2E |
| Backoff monotonicity | Arbitrary params | Monotone increasing | Proptest |
| Backoff lower bound | Arbitrary params | result >= initial | Proptest |
| Backoff no panic | Arbitrary u64,f64,u32 | No panic | Proptest |
| Record transition immutability | Arbitrary record | Only phase changes | Proptest |
| Error classification disjoint | All variants | Not both transient+fatal | Proptest |
| Respawn attempt monotonicity | Arbitrary record | attempts non-decreasing | Proptest |
| Respawn phase reset | Arbitrary record | Phase=Spawn | Proptest |
| Respawn health reset | Arbitrary record | health_checks=0 | Proptest |
| is_zombie consistency | Arbitrary record | Matches exact condition | Proptest |
| should_respawn consistency | Arbitrary record, max | Matches exact condition | Proptest |
| SpawnRecord new defaults | Arbitrary params | Fixed defaults | Proptest |

## 9. Contract Invariant Coverage (ADR-046)

| Contract Invariant | Test Coverage |
|---|---|
| INV-1: Phase Atomicity (exactly one phase) | Proptest: SpawnRecord Transition Immutability |
| INV-2: Attempt Monotonicity | Proptest: Respawn Attempt Monotonicity |
| INV-3: Error Continuity (last_error iff error transition) | Integration: Spawn Failure records error |
| INV-4: PID Binding (spawn_id only in Running) | Integration: Running record has spawn_id |
| INV-5: Single Active Supervisor | Integration: spawn() returns one handle |
| INV-6: Clean Shutdown | Integration: shutdown→ShutDown |
| INV-7: State Broadcast | Integration: State Broadcast test |
| INV-8: Spawn→HealthCheck→Running chain | Integration: Process Cycle Success |
| INV-9: Exponential backoff formula | Unit + Proptest: Backoff calculations |
| INV-10: Cancellation safety | Integration: Shutdown during cycle |
| INV-11: Transient errors logged but don't stop loop | Integration: Error handling in cycle |
| INV-12: Fatal errors logged | Integration: Fatal error handling |

## 10. Observability Test Coverage

| Metric | Increment Trigger | Test |
|---|---|---|
| spawns_successful | Spawn→HealthCheck→Running succeeds | Integration: Process Cycle Success |
| spawns_failed | Spawn fails OR max attempts exceeded | Integration: Spawn Failure, Max Exceeded |
| health_checks_performed | Each health check attempt | Integration: Health Check sequences |
| health_checks_failed | Health check exhausted or errored | Integration: Health Check Failure |
| zombies_detected | Zombie detected (future: not yet in impl) | N/A (implementation gap) |
| respawns | Respawn scheduled | Integration: Health Check Failure with attempts < max |
| dispatch_errors | Storage save fails | Integration: Storage Save Failure |

## 11. Implementation Gaps Noted

These gaps between contract (ADR-046) and implementation should be tracked:

1. **Zombie detection not emitted**: `zombies_detected` metric exists but is never incremented. The `is_zombie` method on `ProcessManager` is never called.
2. **Terminated phase unreachable**: `SpawnPhase::Terminated` exists but no code transitions to it.
3. **WorkQueue unused**: `enqueue_spawn` and `enqueue_resume` are defined but never called by the supervisor.
4. **Shutdown sends no termination signal**: The shutdown path doesn't terminate running processes.
5. **Backoff delay calculated but discarded**: `let _ = backoff_delay;` in line 665 — respawn scheduling is not implemented.
6. **Health check scan uses PID 0**: Health check records scanned by phase use `ProcessHandle { pid: 0, ... }` which is incorrect.

## 12. Test File Location

Tests should be placed in:
- **Unit tests**: `crates/vo-actor/src/spawn_supervisor.rs` (existing `#[cfg(test)] mod tests`)
- **Integration tests**: `crates/vo-actor/tests/spawn_supervisor_integration.rs` (new file)
- **Proptest**: `crates/vo-actor/tests/spawn_supervisor_proptest.rs` (new file)
