# ADR 060 (v2): Thread Affinity Requirements for Critical Loops

## Status
Accepted

## Context
Veloxide runs several long-lived background loops on the tokio runtime's generic worker pool. None currently have thread affinity. When a loop is migrated between OS threads by the work-stealing scheduler, it loses L1/L2 cache residency — every migration is a cold start on the new core.

This matters most for loops that repeatedly access the same memory regions (fjall partition scans, in-memory instance registries). The scheduler has no knowledge of data locality; it only balances compute load. For loops doing sequential key-range scans or batch-draining a write queue, cache warmth is the dominant factor in per-tick latency.

### Audit Findings

Five loops were identified with varying cache-locality sensitivity:

| Priority | Loop | File | Pattern | Cache Impact |
|----------|------|------|---------|--------------|
| CRITICAL | DbWriterActor drain | `vo-actor/src/db_writer.rs` | ractor message handler, batch drain of bounded mpsc | Single-writer for all fjall writes. Cache line bouncing on migration directly increases commit latency. |
| HIGH | Reanimator timer scan | `vo-actor/src/reanimator/loop_core.rs` | `loop { select! { shutdown, tick } }` | Range scan on timers partition every ~1s. Sequential key access benefits from warm L1/L2. |
| HIGH | TimerSupervisor scan | `vo-actor/src/timer_supervisor/supervisor.rs` | `loop { select! { shutdown, tick } }` | Same range-scan pattern as Reanimator on timers partition. |
| MEDIUM | SpawnSupervisor cycle | `vo-actor/src/spawn_supervisor/actor.rs` | `loop { select! { shutdown, tick } }` | Storage reads + process management. Mixed I/O and CPU. |
| MEDIUM | MasterOrchestrator handle | `vo-actor/src/master/lifecycle.rs` | ractor message handler | In-memory HashMap mutations for active instances. Cache-local state access at scale. |

Three additional loops (shutdown waiters in ReanimatorHandle, TimerSupervisorHandle, SpawnSupervisorHandle) and the HeartbeatWatcher are I/O-bound or shutdown-only — affinity provides no benefit.

The `vo-executor` single-threaded runtime (`new_current_thread()`) is already effectively affinitized by virtue of being single-threaded within each subprocess — no action needed.

### Current State
- Zero usage of `core_affinity`, `thread_priority`, `sched_setaffinity`, or any pinning crate anywhere in the workspace.
- All three timer-driven loops use `tokio::runtime::Handle::current().spawn()` — they land on whichever worker thread is available.
- Both ractor actors (DbWriterActor, MasterOrchestrator) run on ractor's internal tokio runtime with no thread affinity control.

## Decision

### 1. Tiered Affinity Policy

Not every loop needs affinity. We apply it only where cache locality is the dominant performance factor, using a two-tier model:

**Tier 1 — Dedicated Core (DbWriterActor only)**
The DbWriterActor is the single write path for the entire engine. It must be pinned to a dedicated core that no other tokio worker uses. This eliminates context switches and guarantees the write batch drain loop always has hot cache.

Implementation: The engine's main tokio runtime is constructed with `worker_threads = N - 1` (where N is total available cores). Core 0 is reserved. A separate single-threaded tokio runtime is created for the DbWriterActor, and its sole thread is affinitized to core 0 via `core_affinity::set_for_current()`.

**Tier 2 — Soft Affinity (Reanimator, TimerSupervisor, SpawnSupervisor, MasterOrchestrator)**
These loops benefit from cache warmth but don't warrant exclusive cores. They use `core_affinity::set_for_current()` at spawn time to pin to a preferred core, but the core is shared with other tokio work. If the OS migrates the thread, performance degrades gracefully — not catastrophically.

### 2. Affinity at Spawn, Not at Runtime

Affinity is set once when the loop task is spawned. There is no re-affinitization logic. If a thread is migrated (e.g., by a signal handler or OS scheduler decision), the loop continues — the cost is a temporary cache miss, not a correctness violation.

### 3. Configuration

Affinity is controlled via engine configuration, not compile-time:

```rust
pub struct AffinityConfig {
    /// Enable thread affinity for critical loops. Default: true on Linux, false otherwise.
    pub enabled: bool,
    /// Core index for the DbWriterActor dedicated core. Default: 0.
    pub db_writer_core: usize,
    /// Preferred cores for Tier-2 loops. Default: [1, 2, 3, 4] (round-robin assigned).
    pub supervisor_cores: Vec<usize>,
}
```

On systems with fewer than 4 cores, Tier 2 affinity is disabled automatically (the scheduler has fewer migration choices anyway). Tier 1 (DbWriterActor) still applies if `enabled` is true.

### 4. Platform Scope

Thread affinity via `sched_setaffinity` is Linux-only. On macOS and Windows:
- `AffinityConfig::enabled` defaults to `false`.
- The configuration is still accepted but silently no-ops.
- The ADR applies to production deployments, which run Linux.

### 5. What Does NOT Get Affinity

- HeartbeatWatcher: network I/O bound, cache locality irrelevant.
- Shutdown wait loops: only active during graceful shutdown.
- Subprocess stderr readers: async I/O, not CPU-bound.
- SDK single-threaded runtime: already single-threaded per subprocess.
- Any future loop that is I/O-bound or runs infrequently.

## Consequences
- **Positive:** DbWriterActor commit latency becomes deterministic — no cache line bouncing on the hottest write path in the system.
- **Positive:** Reanimator and TimerSupervisor range scans retain warm L1/L2 across ticks, reducing per-scan latency.
- **Positive:** Tiered model avoids over-provisioning — only the DbWriterActor gets an exclusive core.
- **Positive:** Configuration-driven — affinity can be disabled for development or non-Linux platforms without code changes.
- **Negative:** Linux-only. No benefit on macOS dev machines. This is acceptable: production is Linux.
- **Negative:** Reserving a core for the DbWriterActor reduces the tokio worker pool by one thread. On machines with 2 cores, this halves available compute. The auto-disable for <4 cores mitigates this.
- **Negative:** `core_affinity` is an external crate. If it becomes unmaintained, the engine needs to replace it with a direct `libc::sched_setaffinity` call — a trivial substitution since the API surface is one function.

## References
- ADR-005: Actor Hibernation and Timer Management (defines the Reanimator loop)
- ADR-011: Asynchronous Task Execution — Current-Thread Runtime (SDK subprocess model)
- ADR-015: Actor Invariants and Backpressure (defines the DbWriterActor bounded mailbox)
- ADR-032: Write-Path QoS and Hot/Cold Storage (defines write classes the DbWriterActor processes)
