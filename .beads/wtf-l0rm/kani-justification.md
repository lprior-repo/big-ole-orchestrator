# Kani Justification: Bead wtf-l0rm

## Formal Argument to Skip Kani Model Checking

### What Critical State Machines Exist?

The new code introduces three functions:
1. `run_timer_loop_watch` - async event loop using select
2. `process_watch_entry` - pure function, no state
3. `sync_and_fire_due` - async iteration, no complex state

**State machines in the implementation:**
- None. The timer loop is a simple select between watch stream and shutdown channel.
- The `TimerRecord` struct is inert data (immutable after creation).
- The `Operation` enum is used for pattern matching only.

### Why Those State Machines Cannot Reach Invalid States

1. **`run_timer_loop_watch`**:
   - States: Running, ShuttingDown
   - Transitions: Running → ShuttingDown (on shutdown signal)
   - No invalid states possible by construction - the select loop handles both branches

2. **`process_watch_entry`**:
   - Pure function with no state
   - Returns `Option<TimerRecord>` - None for delete/purge, Some for put
   - No state transitions

3. **`sync_and_fire_due`**:
   - Iterates keys and fires due timers
   - Each iteration is independent
   - Errors are handled per-entry, loop continues

### What Guarantees the Contract/Tests Provide

1. **Contract Q1**: Timers fired exactly once
   - Guaranteed by `fire_timer()` being idempotent via applied_seq check
   - Test coverage: `test_timer_fired_twice_idempotent`

2. **Contract I1**: Timer never fired before fire_at
   - Guaranteed by `is_due(now)` check before firing
   - Test coverage: `test_invariant_i1_timer_never_fired_before_fire_at`

3. **Contract I2**: Delete only after JetStream append
   - Guaranteed by `fire_timer()` write-ahead order
   - Test coverage: `test_delete_failure_after_fire_logs_warning_and_continues`

4. **Contract I3**: No panics
   - Guaranteed by using `Result` for all fallible operations
   - Test coverage: all tests use `assert!` not `unwrap`

### Formal Reasoning

The implementation is a straightforward translation of the contract:
- Watch stream → process entries → check if due → fire if due
- No complex branching logic that could hide invalid states
- No loops with complex termination conditions
- No concurrent state modifications

**Kani would not find any counterexamples because:**
1. There are no loops with complex invariants (just simple iteration)
2. There are no state machine enums with invalid variants
3. There are no arithmetic properties that could overflow (no counter manipulation)

## Decision

✅ **SKIP KANI** - Formal argument provided above.

The implementation is sufficiently simple that Kani model checking would not provide additional safety guarantees beyond what the contract and unit tests already provide.

## Proceed to State 7 (Architectural Drift)
