# Findings: tw-ve05 - vo-scheduler WorkerDispatch Production Implementation

## Task
Implement production `WorkerDispatch` for vo-scheduler crate. The `WorkerDispatch` trait existed with `RecordingDispatcher` for testing, but no production implementation.

## Changes Made

### 1. Added `DispatchFailed` error variant (`crates/vo-scheduler/src/error.rs`)
Added new error variant to `SchedulerError`:
```rust
#[error("dispatch failed: {0}")]
DispatchFailed(String),
```

### 2. Added `SubprocessWorkerDispatchConfig` struct (`crates/vo-scheduler/src/scheduler.rs`)
Configuration struct holding:
- `executable_path: std::path::PathBuf` - path to worker binary
- `timeout_ms: u64` - timeout for subprocess execution (stored but not yet enforced)

### 3. Added `SubprocessWorkerDispatch` implementing `WorkerDispatch` trait
Production dispatcher that:
- Spawns the configured executable with job payload as command-line arguments
- Waits synchronously for subprocess completion
- Returns `Ok(())` on successful exit (exit code 0)
- Returns `Err(SchedulerError::DispatchFailed(...))` on non-zero exit or spawn failure
- Uses `std::process::Command` (no async/tokio dependency required)

### 4. Exported new types from `lib.rs`
Added `SubprocessWorkerDispatch` and `SubprocessWorkerDispatchConfig` to public exports.

### 5. Added unit tests
- `subprocess_dispatch_successful_execution` - tests `/bin/echo` succeeds
- `subprocess_dispatch_nonexistent_binary` - tests error handling for missing binary
- `subprocess_dispatch_failing_subprocess` - tests error handling for `/bin/false`
- `subprocess_dispatch_with_args` - tests payload parsed as shell args
- `subprocess_config_debug_and_clone` - tests Debug and Clone
- `subprocess_dispatch_config_accessors` - tests accessor methods

## Architecture Notes

### Design Decision: Synchronous Execution
The implementation uses `std::process::Command` for synchronous subprocess execution. This was chosen because:
1. `vo-scheduler` doesn't have tokio as a runtime dependency
2. `WorkerDispatch::dispatch` is a synchronous method
3. Simpler than thread-spawn approaches

### Limitation: Synchronous Blocking
The current implementation blocks the calling thread until the subprocess completes. For high-throughput scenarios, an async implementation using `tokio::process::Command` would be better, but would require restructuring the scheduler to use async dispatch.

### Payload Interpretation
The job's `payload: SerializedPayload` (bytes::Bytes) is interpreted as shell command-line arguments, split by whitespace. For example, payload `"-c echo hello"` becomes arguments `["-c", "echo", "hello"]`.

### Timeout Configuration
The `timeout_ms` config field is stored but not currently enforced. A future improvement would implement actual timeout enforcement using `std::thread::sleep` in a spawn or similar approach.

## Verification
- All 397 tests pass (6 new tests for the implementation)
- `cargo clippy` shows 0 errors, only pre-existing warnings in vo-types
- `cargo fmt` applied successfully
- `cargo build` succeeds without warnings

## Files Modified
- `crates/vo-scheduler/src/error.rs` - Added DispatchFailed variant
- `crates/vo-scheduler/src/scheduler.rs` - Added SubprocessWorkerDispatch* types and tests
- `crates/vo-scheduler/src/lib.rs` - Added exports
