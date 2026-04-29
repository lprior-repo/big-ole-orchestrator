# Findings: tw-wmox - Add panic recovery for actor supervisor

## Summary
Implemented panic recovery infrastructure for ractor actors in the vo-actor crate.

## What Was Done

### 1. Created `vo-actor/src/actor_supervisor/` module
New module with the following components:

#### `types.rs` - Core types for actor supervision
- `ActorSupervisorConfig`: Configuration for supervisor behavior (max_restart_attempts, backoff settings)
- `ActorSupervisorState`: Tracks actor state, restart attempts, last panic/restart times
- `PanicInfo`: Captured panic information including backtrace
- `ActorSupervisorError`: Error types for panic, max restarts exceeded, and isolation
- `RestartDecision`: Enum for restart decisions (RestartNow, RestartWithBackoff, Isolate, NoRestart)
- `compute_restart_decision()`: Pure function to decide restart strategy

#### `metrics.rs` - Metrics for monitoring
- `ActorSupervisorMetrics`: Atomic counters for panics, restarts, isolations, permanent failures
- Metric emission functions for observability

#### `audit.rs` - Audit logging
- `ActorSupervisorAuditEntry`: Structured audit entries for all supervisor events
- `AuditLog` trait for pluggable audit backends
- `NoOpAuditLog` implementation
- `emit_audit_log()` function for tracing-based audit

#### `panic_catcher.rs` - Panic catching utilities
- `PanicCatcher::catch_panic()`: Wraps operations to catch panics
- `PanicCatcher::catch_panic_with_backtrace()`: Returns backtrace along with error
- `log_panic_with_backtrace()`: Full backtrace logging via tracing
- `capture_panic_info()`: Extracts panic message and backtrace

### 2. Updated `vo-actor/src/lib.rs`
Added `pub mod actor_supervisor;` to export the new module.

## Key Design Decisions

1. **Data → Calc → Actions pattern**: Follows the existing codebase conventions
2. **Panic catching via `std::panic::catch_unwind`**: Standard Rust mechanism
3. **Backtrace capture**: Uses `std::backtrace::Backtrace::capture()` for full backtraces
4. **Thread-safe metrics**: Uses `AtomicU64` for lock-free counters
5. **Audit trail**: Structured entries for compliance and debugging
6. **Isolation after max attempts**: Prevents cascading failures by isolating repeatedly panicking actors

## How It Works

1. When an actor operation might panic, wrap it with `PanicCatcher::catch_panic()`
2. On panic:
   - Metrics are incremented
   - Panic info (message + backtrace) is captured
   - Audit log entry is created
   - Error is returned with full context
3. Supervisor decides restart strategy based on:
   - Number of previous restart attempts
   - Configured max attempts
   - Exponential backoff between restarts
4. If max attempts exceeded, actor is isolated to prevent cascading failures

## Files Changed
- `crates/vo-actor/src/lib.rs` - Added actor_supervisor module
- `crates/vo-actor/src/actor_supervisor/mod.rs` - New file
- `crates/vo-actor/src/actor_supervisor/types.rs` - New file
- `crates/vo-actor/src/actor_supervisor/metrics.rs` - New file
- `crates/vo-actor/src/actor_supervisor/audit.rs` - New file
- `crates/vo-actor/src/actor_supervisor/panic_catcher.rs` - New file

## Status
- [x] Library compiles successfully
- [x] All new code follows codebase conventions
- [ ] Tests compile (pre-existing broken tests in vo-actor block test build)
- [x] Implementation provides full panic recovery infrastructure

## Notes
The pre-existing test compilation issues in vo-actor (probe module, vo_actor_comprehensive_tests) are unrelated to this change and block running the unit tests for this module. The library itself compiles cleanly.