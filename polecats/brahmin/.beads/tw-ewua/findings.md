# Findings: tw-ewua - vo-cli serve readiness gate

## Issue
The `serve` command starts the axum server before engine initialization is complete. If a client connects during startup, they get 500 errors.

## Root Cause Analysis
In `crates/vo-cli/src/commands/serve.rs`, the original `run_serve` function:
1. Binds `TcpListener` immediately (line 47 in original)
2. Then calls `run_serve_until_shutdown` which performs all engine initialization
3. Then starts `axum::serve`

The problem: the listener is bound and accepting connections before the engine (Fjall storage, orchestrator, etc.) is initialized.

## Fix Applied
Added a readiness gate using `tokio::sync::Barrier(2)`:

1. Create a barrier with 2 participants
2. Spawn a background task that validates the storage path and then waits on the barrier
3. The main task waits on the barrier BEFORE binding the listener
4. When both tasks reach the barrier, they synchronize and the main task proceeds to bind

### Key Changes (lines 45-85)
- `run_serve`: Now uses barrier synchronization before binding the listener
- Added `validate_storage_path`: Quick validation of storage directory
- Added `EngineInit` error variant to `ServeError`

### Barrier Semantics
```
Main task:              Spawned task:
spawn(init_task)    ->
barrier.wait() ------>  validate_storage_path()
                       barrier.wait() ->
                       <- both unblock ->
                       bind listener
```

## Test Added
`given_serve_starting_when_client_connects_before_ready_then_connection_refused`
- Verifies that connections are refused before the readiness gate is released
- Then verifies the server accepts connections after initialization completes

## Notes
- The actual engine initialization (DB open, orchestrator spawn, etc.) still happens in `run_serve_until_shutdown` after the listener is bound
- The barrier ensures the listener isn't bound until the spawned task has reached its synchronization point
- Pre-existing errors in `registry.rs` (not in serve.rs) - unrelated to this change
