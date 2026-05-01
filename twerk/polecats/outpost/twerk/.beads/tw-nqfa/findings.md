# Findings: tw-nqfa - Engine Lifecycle Tests

## Bead Description
Test engine lifecycle transitions through Starting->Running->Stopping->Stopped

## Investigation

### Actual State Machine (from state.rs)
The Engine uses these states:
- `Idle` (default, corresponds to bead's "Starting")
- `Running`
- `Terminating` (corresponds to bead's "Stopping")
- `Terminated` (corresponds to bead's "Stopped")

### Error Types (from engine_lifecycle.rs)
- `EngineError::NotIdle` - returned when start() fails (bead calls this "AlreadyRunning")
- `EngineError::NotRunning` - returned when terminate() fails

### Existing Tests Found

**In `engine_lifecycle_test.rs`:**
- `engine_new_creates_idle_engine` - verifies new engine is Idle
- `engine_start_fails_when_not_idle` - verifies start() when Running returns Err(NotIdle)
- `engine_terminate_fails_when_not_running` - verifies terminate() when not Running returns Err
- `start_standalone_initializes_broker_and_datastore` - full lifecycle test
- `engine_shutdown_*` tests - shutdown behavior

**In `bdd_behavioral_contracts.rs`:**
- `only_terminated_is_terminal` - verifies Terminated is the only terminal state
- `idle_can_transition_to_any_state` - Idle can transition anywhere
- `running_can_only_transition_to_terminating` - Running → Terminating only
- `terminating_can_only_transition_to_terminated` - Terminating → Terminated only
- `terminated_cannot_transition` - Terminated is a dead end
- `full_state_lifecycle` - explicit Idle → Running → Terminated test

### Mapping: Bead Terminology → Implementation

| Bead Description | Actual Implementation |
|-------------------|----------------------|
| new engine -> Starting | new engine -> Idle |
| start() -> Running | start() -> Running ✓ |
| stop() -> Stopping -> Stopped | terminate() -> Terminating -> Terminated |
| stop() when Stopped -> no-op | terminate() when Terminated -> Err(NotRunning) |
| start() when Running -> Err(AlreadyRunning) | start() when Running -> Err(NotIdle) |
| force stop during Starting -> Stopped | N/A - no "Starting" intermediate state |

## Conclusion

The lifecycle tests described in this bead ALREADY EXIST in the codebase:
1. New engine starts in Idle (tested)
2. start() transitions to Running (tested)
3. terminate() transitions to Terminated (tested)
4. terminate() when not Running returns error (tested)
5. start() when not Idle returns error (tested)

The bead terminology differs from implementation, but the functionality is fully covered by existing tests in `engine_lifecycle_test.rs` and `bdd_behavioral_contracts.rs`.

**Recommendation**: No new tests needed - close as completed since tests already exist.