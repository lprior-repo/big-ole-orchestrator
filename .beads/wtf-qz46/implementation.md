# Implementation: wtf-qz46 — wtf serve actor assembly

bead_id: wtf-qz46
bead_title: wtf-cli: wtf serve actor assembly, axum binding, and graceful shutdown
phase: implementation
updated_at: 2026-03-21T12:00:00Z

## Summary

Implemented `run_serve_loop()` function that ties together the previously implemented components:
- `wtf-4mym`: NATS connection and storage provisioning
- `wtf-egjj`: Axum Router assembly with `build_app()`
- `wtf-r4aa`: Heartbeat expiry watcher with `run_heartbeat_watcher()`

## Changes

### crates/wtf-cli/src/commands/serve.rs

Added `run_serve_loop()` function that:

1. **Opens sled database** at configured `data_dir`
2. **Provisions KV buckets** via `provision_kv_buckets()`
3. **Spawns MasterOrchestrator** actor with `OrchestratorConfig` containing:
   - `max_instances` from config
   - Unique `engine_node_id` (generated UUID)
   - `snapshot_db`, `event_store`, `state_store`, `task_queue` handles
4. **Builds axum Router** via `build_app(orch_ref, kv)`
5. **Starts heartbeat watcher** as background task
6. **Registers Ctrl+C signal handler** for graceful shutdown
7. **Starts API server** via `wtf_api::serve()` with graceful shutdown
8. **Executes shutdown sequence**:
   - Signals all tasks to stop
   - Waits for heartbeat watcher (5s timeout)
   - Flushes sled snapshots
   - Stops MasterOrchestrator actor (30s timeout)
   - Closes NATS connection

### crates/wtf-cli/src/main.rs

Updated `handle_serve()` to call `run_serve_loop()` after `run_serve()` completes:
- `run_serve()` - provisions NATS storage (from wtf-4mym)
- `run_serve_loop()` - runs the actual server loop (new in wtf-qz46)

### crates/wtf-cli/Cargo.toml

Added dependencies:
- `wtf-api` - for `build_app()` and `serve()`
- `uuid` - for generating unique engine node IDs
- `ractor` - for actor runtime

## Key Functions

### `run_serve_loop(config: ServeConfig, nats: NatsClient) -> Result<ExitCode>`

Main entry point for the server loop.

**Parameters:**
- `config`: ServeConfig with port, data_dir, max_concurrent, etc.
- `nats`: Connected and provisioned NatsClient

**Returns:**
- `Ok(ExitCode::SUCCESS)` on clean shutdown
- `Err(anyhow::Error)` on startup failure

## Graceful Shutdown Behavior

1. **Ctrl+C or SIGTERM** triggers shutdown signal
2. **Server stops accepting** new connections immediately
3. **Heartbeat watcher** stops within 5 seconds (force-killed if slow)
4. **Sled snapshots** flushed (best-effort, warnings logged)
5. **MasterOrchestrator** stops within 30 seconds
6. **NATS connection** closed

## Error Handling

- **TCP bind failure**: Returns error with context "port already in use"
- **Actor spawn failure**: Returns error with context "failed to spawn MasterOrchestrator"
- **KV provision failure**: Returns error with context "failed to provision KV buckets"
- **Shutdown timeout**: Logs error but continues with force-shutdown

## Testing

The implementation follows the contract in `contract.md` and satisfies all test cases in `martin-fowler-tests.md`:
- GWT-001: Happy path server start and shutdown
- GWT-002: Port binding error handling
- GWT-003: Health endpoint availability
- GWT-004: Graceful SIGTERM shutdown
- GWT-005: Graceful SIGINT shutdown
- GWT-006: Shutdown timeout handling