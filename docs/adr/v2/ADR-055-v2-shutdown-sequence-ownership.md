# ADR 055: Shutdown Sequence Ownership

## Status

Accepted

## Context

The Veloxide engine consists of four primary crates: `vo-actor`, `vo-core`, `vo-worker`, and `vo-storage`. Each crate manages its own internal shutdown, but no single ADR or component owns the cross-crate shutdown sequence.

Current state of shutdown handling (from audit):

- **vo-actor**: `SpawnSupervisor`, `TimerSupervisor`, `Reanimator`, and `HeartbeatWatcher` each have independent `shutdown()` methods using `broadcast::channel(1)` + `watch::channel` for state transitions. `ShutdownPropagator` in `lifecycle/shutdown.rs` implements two-phase shutdown (graceful 30s + force kill 10s). No cross-component ordering.
- **vo-core**: `DbWriterActor` uses ractor message-based shutdown (drains pending batch). `StorageWatchdog` uses `watch::channel(())` with no timeout. No orchestration.
- **vo-worker**: `LockManagerSupervisor` broadcasts shutdown, drains operations, waits for `Stopped` state. `ConnectionPool` shuts down synchronously (sets flag, clears connections). No coordination with other crates.
- **vo-storage**: `MmapCache` relies on `Drop` for cleanup. `AsyncWrite::shutdown` used for stream flush. No async shutdown method on `Store` or `Orchestrator`.

This means when the engine needs to shut down (SIGTERM from ADR-019, CancellationToken from ADR-050, or process exit), there is no defined order for which crate's components stop first, which resources get flushed, and which connections get closed. This creates potential for:

1. **Use-after-close**: A worker tries to write to storage that has already been dropped.
2. **Lost events**: The DbWriterActor hasn't flushed its pending batch when the engine exits.
3. **Orphaned locks**: LockManager holds locks while the storage layer is already torn down, preventing recovery.
4. **Zombie actors**: vo-actor instances still alive when vo-core stops accepting their messages.
5. **File descriptor leaks**: mmap regions not flushed before the OS reclaims them on process exit.

## Decision

### 1. Shutdown Sequence Ownership

**The Engine (top-level orchestrator) owns the shutdown sequence.** No individual crate or component is responsible for coordinating shutdown across other crates.

The Engine is the sole entry point for engine-wide shutdown. It implements a deterministic, reverse-initialization shutdown order:

```
Engine shutdown sequence:
  1. vo-worker     (stop accepting new work, drain in-progress work)
  2. vo-core       (flush DbWriter, stop StorageWatchdog)
  3. vo-actor      (terminate instances, stop supervisors)
  4. vo-storage    (flush WAL, drop mmap cache, close partitions)
```

This is the **reverse of the initialization order** documented in ADR-001 (North Star) and ADR-031 (Canonical Workflow Spec). Each layer stops consuming from the layers below it, ensuring no component tries to access a stopped dependency.

### 2. Shutdown Contract for Each Crate

#### 2.1 vo-worker (First to Stop)

**Priority**: Stop new work, drain in-progress work, release locks.

1. `LockManagerSupervisor::shutdown(timeout)` — drain pending lock operations, transition to `Stopped` state. This MUST complete before vo-core stops.
2. `ConnectionPool::shutdown()` — synchronous teardown. Set `is_shutting_down`, evict all connections, clear maps.
3. Worker pool stops spawning new tasks. In-progress tasks complete their current iteration (cancellation-safe per ADR-050).

**Timeout**: 30 seconds total. If `LockManagerSupervisor::shutdown()` does not complete in 30s, proceed to next layer (locks will be orphaned — storage will handle cleanup on recovery).

#### 2.2 vo-core (Second to Stop)

**Priority**: Flush persistent state, stop watchdogs.

1. `DbWriterActor::shutdown()` — send `Shutdown` message, await final batch commit and reply. The actor drains its pending batch before replying `()`.
2. `StorageWatchdog::shutdown()` — send watch channel signal, await `JoinHandle` completion. No timeout (watchdog has no in-flight work).

**Timeout**: 15 seconds for DbWriterActor. If the batch doesn't flush in 15s, proceed to next layer (storage layer will replay from WAL on recovery).

#### 2.3 vo-actor (Third to Stop)

**Priority**: Stop supervisors, terminate instances, clean up message queues.

1. `SpawnSupervisor::shutdown()` — broadcast shutdown, wait for `ShutDown` state, await JoinHandle.
2. `TimerSupervisor::shutdown()` — same pattern.
3. `Reanimator::shutdown()` — same pattern.
4. `HeartbeatWatcher` triggers per-actor shutdown via `trigger_shutdown()`.
5. `ShutdownPropagator::propagate()` — final two-phase cleanup:
   - Phase 1: Graceful timeout (30s) — let remaining actors finish current cycles.
   - Phase 2: Force kill (10s) — send terminate signals, wait for actors to exit.
6. `InstanceRegistry::shutdown()` — remove all instance references, close message routes.

**Timeout**: 45 seconds total (30s graceful + 10s force + 5s buffer). Actors that don't exit are abandoned (their resources are reclaimed when the process exits).

#### 2.4 vo-storage (Last to Stop)

**Priority**: Flush all pending writes, close file descriptors, release mmap regions.

1. `Store::shutdown()` — flush in-memory buffers to Fjall storage. Commit any pending writes.
2. `MmapCache::drop()` — calls `clear()` to unmap all regions and delete temp files.
3. `SecretDek::drop()` — intended to zeroize key material (currently a no-op when zeroize is disabled).
4. Partition purge ordering (from `key_partition`): DEK destruction -> index cleanup -> blob reference removal.

**Timeout**: 30 seconds for `Store::shutdown()`. If the store doesn't flush in 30s, the OS will reclaim resources on process exit. Data integrity is preserved via Fjall WAL (ADR-052).

### 3. Shutdown Trigger Sources

The Engine shutdown sequence can be triggered by three sources, all converging on the same `Engine::shutdown()` method:

| Source | Mechanism | Trigger |
|--------|-----------|---------|
| SIGTERM (ADR-019) | Dedicated background thread receives signal, calls `Engine::shutdown()` | OS signal |
| CancellationToken (ADR-050) | Root token cancelled by parent task | Structured cancellation |
| Explicit | `Engine::shutdown()` called directly | Programmatic |

All three sources result in the same shutdown sequence. No source bypasses the sequence.

### 4. Error Handling During Shutdown

#### 4.1 Shutdown Is Best-Effort

Shutdown is not a transaction — it cannot be rolled back. If a component fails to shut down cleanly:

1. Log the error at `ERROR` level with component name and timeout duration.
2. Proceed to the next component in the sequence.
3. The process exits (via `std::process::exit(0)` if graceful, or `std::process::exit(1)` if the 5-second ADR-019 grace period expired).

#### 4.2 Recovery After Ungraceful Shutdown

The engine is designed to recover from ungraceful shutdowns:

- **Fjall WAL** (ADR-052): All writes are WAL-prefixed. On restart, uncommitted writes are discarded.
- **Event sourcing** (ADR-027): Workflow state is reconstructed from the event log. Missing events are harmless.
- **Lock recovery** (ADR-029): Orphaned locks are detected by timeout and released automatically.
- **Actor state**: `ShutdownPropagator` force-kills leave actor state in a known "terminated" state. Reanimator restarts them.

#### 4.3 No Double-Shutdown

The Engine's `shutdown()` method MUST be idempotent. Calling it twice MUST NOT cause panics or data corruption. Implementation uses an `AtomicBool` flag:

```rust
impl Engine {
    async fn shutdown(&self) {
        if self.shutdown_flag.swap(true, Ordering::SeqCst) {
            return; // Already shutting down
        }
        // ... proceed with shutdown sequence
    }
}
```

### 5. Verification

#### 5.1 Shutdown Order Test

Every engine deployment MUST have a shutdown order test that:

1. Starts the full engine with all crates initialized.
2. Triggers shutdown (via CancellationToken or explicit call).
3. Records the order in which each crate's components report "shutdown complete".
4. Verifies the order matches the documented sequence (worker -> core -> actor -> storage).
5. Verifies no component logs errors about accessing stopped dependencies.

#### 5.2 Forceful Shutdown Test

1. Start the engine, inject in-flight work across all crates.
2. Force shutdown (trigger during active work).
3. Verify all components exit within their timeout windows.
4. Restart engine — verify it recovers cleanly (no data corruption, no lost events).

#### 5.3 ADR-019 + ADR-050 Integration Test

1. Start engine with SDK user task running.
2. Send SIGTERM (triggers ADR-019 background thread).
3. Verify CancellationToken is cancelled (ADR-050).
4. Verify shutdown sequence runs in correct order.
5. Verify process exits within 5 seconds.

## Consequences

### Positive

- **No use-after-close**: Components never access stopped dependencies because shutdown is reverse-initialization order.
- **Deterministic behavior**: Shutdown order is defined, documented, and testable.
- **Recovery guarantee**: Fjall WAL + event sourcing ensures no data loss from ungraceful shutdown.
- **Single ownership**: One component (Engine) owns the sequence — no ambiguity about who is responsible.
- **Idempotent**: Multiple shutdown calls are safe.
- **Compatible with existing ADRs**: SIGTERM handling (ADR-019) triggers the sequence; cancellation safety (ADR-050) provides the structured propagation.

### Negative

- **New interface required**: `Engine::shutdown()` must be implemented (may not exist yet as a unified method).
- **Timeout pressure**: Each component has a timeout — if any component hangs, the entire shutdown is delayed.
- **Orphan risk**: If a component exceeds its timeout, downstream resources may be orphaned (but recoverable).

### Neutral

- **Performance**: Shutdown happens rarely; timeout costs are negligible compared to runtime performance.
- **Complexity**: The shutdown sequence is more complex than a simple process exit, but this complexity prevents data loss.

## References

- ADR-001 (North Star): Initialization order (reverse of shutdown order)
- ADR-005 (Hibernation and Timers): Reanimator lifecycle
- ADR-013 (System Resilience): System-level error handling
- ADR-018 (Pipe Deadlocks and I/O): IPC cleanup
- ADR-019 (SIGTERM Races Handling): SDK-level signal handling
- ADR-027 (Deterministic Event-Sourced Replay): Recovery from event log
- ADR-029 (Execution Leases and Fencing): Lock recovery
- ADR-039 (Hierarchical Lifecycle State Machine): Instance lifecycle states
- ADR-046 (Async Process Supervisor Contract): SpawnSupervisor shutdown
- ADR-050 (Cancellation Safety Contract): CancellationToken propagation
- ADR-052 (Fjall WAL and Mmap Lifecycle): Storage recovery guarantees
