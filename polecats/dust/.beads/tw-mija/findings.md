# Bead tw-mija Findings: Add Structured Tracing to vo-actor State Transitions

## Task
Add `tracing::info` spans to every actor state transition in vo-actor with structured fields: `instance_id`, `from_state`, `to_state`, `event_type`, `duration`.

## Changes Made

### 1. spawn_supervisor/actor.rs
Added structured tracing spans for `SpawnSupervisorState` transitions:
- **Stopped → Running** (event: `spawn`) - in `spawn()` method
- **Running → ShuttingDown** (event: `shutdown_signal`, includes `duration_ms`) - in `run_loop()`
- **Running → ShutDown** (event: `loop_exit`, includes `duration_ms`) - in `run_loop()`

### 2. spawn_supervisor/cycle.rs
Added structured tracing spans for `SpawnPhase` transitions on spawn records:
- **Spawn → HealthCheck** (event: `spawn_success`) - after process spawn succeeds
- **HealthCheck → Running** (event: `health_check_passed`) - after health check passes (2 locations)
- **HealthCheck → Failed** (event: `health_check_failed`) - after health check fails (2 locations)
- **Running → Failed** (event: `zombie_detected`) - on zombie process detection
- **Failed → Quarantined** (event: `quarantined`) - after consecutive failure threshold
- **Failed → Spawn** (event: `respawn`) - when respawning a failed process

### 3. timer_supervisor/supervisor.rs
Added structured tracing spans for `TimerSupervisorState` transitions:
- **Stopped → Running** (event: `spawn`) - in `spawn()` method
- **Running → ShuttingDown** (event: `shutdown_signal`, includes `duration_ms`) - in `run_loop()`
- **Running → ShutDown** (event: `loop_exit`, includes `duration_ms`) - in `run_loop()`

### 4. Supporting Changes
Added `Display` implementations for:
- `SpawnSupervisorState` (crates/vo-actor/src/spawn_supervisor/types.rs)
- `TimerSupervisorState` (crates/vo-actor/src/timer_supervisor/types.rs)

Required because tracing span fields with `%` format require `Display` trait.

## Structured Fields Used
- `instance_id` - Used in spawn_phase_transition spans (SpawnRecord.instance_id)
- `from_state` - Previous state (enum as Display)
- `to_state` - New state (enum as Display)
- `event_type` - String describing the transition reason (e.g., "spawn", "shutdown_signal", "health_check_passed")
- `duration_ms` - Time in milliseconds since actor start (only on shutdown/loop_exit transitions)

## Files Modified
- `crates/vo-actor/src/spawn_supervisor/actor.rs` (+25 lines)
- `crates/vo-actor/src/spawn_supervisor/cycle.rs` (+72 lines)
- `crates/vo-actor/src/spawn_supervisor/types.rs` (+11 lines - Display impl)
- `crates/vo-actor/src/timer_supervisor/supervisor.rs` (+25 lines)
- `crates/vo-actor/src/timer_supervisor/types.rs` (+10 lines - Display impl)

## Verification
- `cargo check --package vo-actor` passes
- Tests have pre-existing compilation errors unrelated to these changes

## Span Names
- `spawn_supervisor.state_transition` - For SpawnSupervisor actor state changes
- `spawn_phase_transition` - For SpawnRecord SpawnPhase changes
- `timer_supervisor.state_transition` - For TimerSupervisor actor state changes
