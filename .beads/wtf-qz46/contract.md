# Contract: wtf-qz46 — wtf serve actor assembly, axum binding, and graceful shutdown

bead_id: wtf-qz46
bead_title: wtf-cli: wtf serve actor assembly, axum binding, and graceful shutdown
phase: contract
updated_at: 2026-03-21T04:27:00Z

## Context

Bead wtf-4mym provides a ready `NatsClient` with provisioned JetStream streams and KV buckets. Bead wtf-egjj provides the `build_app()` function to assemble the axum Router. Bead wtf-r4aa provides the `run_heartbeat_watcher()` function for heartbeat expiry detection.

This bead assembles the complete run loop:
1. Spawn MasterOrchestrator actor
2. Assemble axum Router with all API routes
3. Bind TCP listener on configured port
4. Start heartbeat watcher polling loop
5. Register signal handlers (SIGTERM/SIGINT)
6. Await shutdown signal
7. Drain in-flight actors
8. Flush sled snapshots
9. Close NATS connection

## Preconditions

- `NatsClient` is connected and JetStream/KV buckets are provisioned
- `sled_db` is opened at `data_dir`
- `OrchestratorConfig` is constructed with all required stores
- `port` is a valid TCP port (1024-65535)

## Postconditions

### Happy Path

1. **Actor Spawned**: `MasterOrchestrator` actor is spawned and responsive
2. **Router Assembled**: `build_app(orch_ref, kv)` produces a `Router` with all API routes
3. **TCP Bound**: Axum server is listening on `host:port`
4. **Heartbeat Watcher Running**: Background task watching `wtf-heartbeats` KV bucket
5. **Signal Handlers Registered**: SIGTERM/SIGINT trigger graceful shutdown
6. **Shutdown Sequence**:
   - Server stops accepting new connections
   - In-flight actors are drained (with timeout)
   - Sled snapshots are flushed
   - NATS connection is closed gracefully
7. **Server Exits**: Returns `ExitCode::SUCCESS` after clean shutdown

### Error Paths

1. **Actor Spawn Failure**: Return `ExitCode::FAILURE` with error message
2. **TCP Bind Failure**: Return `ExitCode::FAILURE` with "port already in use" context
3. **Heartbeat Watcher Failure**: Log error, continue serving (degrade gracefully)
4. **Shutdown Timeout**: Force-kill actors after 30s timeout, flush snapshots, exit anyway
5. **Sled Flush Failure**: Log warning, continue with NATS close

## Contract Summary

```
run_serve_loop(config: ServeConfig, nats: NatsClient, sled_db: Arc<sled::Db>, kv: KvStores) -> Result<ExitCode>
```

### Inputs
- `config.port: u16` — TCP port to bind
- `config.max_concurrent: usize` — max workflow instances
- `nats: NatsClient` — connected NATS client with JetStream
- `sled_db: Arc<sled::Db>` — sled snapshot database
- `kv: KvStores` — NATS KV bucket handles

### Outputs
- `Ok(ExitCode::SUCCESS)` — clean shutdown
- `Err(anyhow::Error)` — startup or runtime error

## Implementation Notes

- Use `tokio::signal::ctrl_c()` or `tokio::signal::unix::signal()` for signal handling
- Use `axum::serve()` with `with_graceful_shutdown()` for HTTP shutdown
- Use `tokio::select!` to coordinate between signal and shutdown triggers
- Sled flush via `sled_db.flush()` or `sled_db.flush_async()`
- Actor drain: send `Stop` to all active instances, await their termination handles

## Dependencies

- `wtf-4mym`: NatsContext (CLOSED)
- `wtf-egjj`: build_app() (CLOSED)
- `wtf-r4aa`: run_heartbeat_watcher() (CLOSED)