# vel-fsm0: SpawnSupervisor Zombie Detection Fix - Findings

## Issue
SpawnSupervisor zombie detection was never invoked despite being defined in ADR-046 (Async Process Supervisor Contract).

## Root Cause
Three zombie detection mechanisms existed but were never called:

1. **`is_zombie_state(record)`** - Pure function in `pure.rs` that checks if a SpawnRecord is in Failed phase with >3 attempts. NEVER called in `cycle.rs`.

2. **`process_manager.is_zombie(pid)`** - Trait method on `ProcessManager` that checks if a process is actually a zombie at the OS level. NEVER called anywhere.

3. **`zombies_detected` metric** - Counter in `metrics.rs` that was never incremented.

## Impact
When a process repeatedly failed to spawn (Failed phase + >3 attempts), the supervisor would keep attempting respawn forever without ever checking if the process was actually a zombie. This wasted CPU cycles and prevented proper error handling for genuinely broken processes.

## Fix Applied

### `crates/vo-actor/src/spawn_supervisor/cycle.rs`
Added zombie detection check in Phase 3 (respawn failed spawns), before the `should_respawn` check:

1. Imported `is_zombie_state` from `pure` module
2. Added check: when processing a Failed record that matches zombie state criteria (Failed phase + >3 attempts):
   - Extract PID from `last_error` (`ProcessExited` variant)
   - Call `process_manager.is_zombie(pid)` to verify zombie status
   - If zombie: increment `zombies_detected` metric, log warning, skip respawn (continue)
   - If not zombie or check fails: proceed with normal respawn logic

### `crates/vo-actor/tests/spawn_supervisor_integration.rs`
Updated the integration test `process_cycle_increments_zombies_detected_metric`:
- Changed expected assertion from `zombies_detected == 0` (documenting the gap) to `zombies_detected == 1` (verifying the fix)
- Removed "IMPLEMENTATION GAP" comments

## Additional Fixes (Pre-existing)
Fixed pre-existing compilation errors in test files that were blocking test execution:
- Fixed `ExecutionSemaphore` import paths (moved from `spawn_supervisor` to `crate::semaphore`)
- Fixed `SpawnRecord::new()` call signatures (wrong number of arguments, wrong type for executable parameter)
- Fixed `SpawnFailed` variant field name (`command` -> `executable`)
- Fixed `CancelError::AlreadyTerminal` and `AcceptResumeError::InvalidLifecycleState` pattern matches (missing `..` for extra fields)

## Test Results
- `cargo test -p vo-actor --test spawn_supervisor_integration`: 52 passed
- `cargo test -p vo-actor --test spawn_supervisor_proptest`: 14 passed
- `cargo test -p vo-actor --lib -- spawn_supervisor`: 21 passed
- All spawn supervisor tests pass

## Architecture Alignment
This fix aligns the implementation with ADR-046 which defines:
- ProcessManager trait with `is_zombie(pid)` method
- `ZombieDetected` fatal error variant  
- `is_zombie_state()` pure function for state-based detection
- `zombies_detected` metric for monitoring

The implementation now correctly invokes zombie detection during the respawn cycle, closing the gap identified in the ADR gap audit.
