# Contract Specification: Timer Loop KV Watch Optimization

## Context
- **Feature**: Optimize timer loop to use KV watch instead of polling
- **Bead ID**: wtf-l0rm
- **Domain terms**:
  - `wtf-timers` - NATS KV bucket storing pending timers
  - `TimerRecord` - msgpack-encoded timer data (timer_id, namespace, instance_id, fire_at)
  - `poll_and_fire` - current polling function that lists all keys, fetches all values
  - `watch_all()` - async_nats KV watch stream that yields change operations
  - `Operation::Put` - new entry created
  - `Operation::Delete` - entry deleted
  - `TimerFired` - JetStream event appended when timer fires
- **Assumptions**:
  - async_nats KV watch stream provides initial snapshot of all entries
  - Timer firing semantics remain identical (write-ahead, idempotent)
  - Shutdown handling remains unchanged
- **Open questions**:
  - Does watch_all() include existing entries on initial connection?

## Preconditions
- [P1] NATS KV store (`wtf-timers`) is accessible and watchable
- [P2] JetStream context is available for appending `TimerFired` events
- [P3] `shutdown_rx` channel is valid and receives shutdown signals

## Postconditions
- [Q1] All timers that are due are fired exactly once (idempotent via applied_seq)
- [Q2] Timer loop continues processing until shutdown signal received
- [Q3] All KV entries that were due at connection time are eventually processed (initial sync)
- [Q4] No redundant KV operations (keys() called only for initial sync, not per-interval)

## Invariants
- [I1] A timer is never fired before its `fire_at` time
- [I2] A timer is removed from KV only after `TimerFired` is appended to JetStream
- [I3] The timer loop does not panic; errors are logged and loop continues

## Error Taxonomy
- `WtfError::NatsPublish` - JetStream append failure
- `WtfError::NatsKv` - KV operation failure (get, put, delete, watch)
- `WtfError::Serialization` - msgpack encode/decode failure

## Contract Signatures

### New/Modified Functions

```rust
// Optimized: uses watch instead of polling
pub async fn run_timer_loop_watch(
    js: Context,
    timers: Store,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), WtfError>

// Unchanged signature, but now called on watch events
pub async fn fire_timer(...) -> Result<u64, WtfError>
pub async fn delete_timer(...) -> Result<(), WtfError>
```

## Type Encoding

| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| NATS accessible | Runtime-checked | `Result<...>` on watch_all |
| Valid shutdown_rx | Compile-time | `tokio::sync::watch::Receiver<bool>` |
| Timer due check | Runtime-checked | `record.is_due(now)` guard |
| Serialization | Runtime-checked | `Result<T, WtfError>` |

## Violation Examples (REQUIRED)

- VIOLATES Q1: `watch stream skips entry due at t=0` → should fire when watch delivers entry (eventual consistency)
- VIOLATES Q3: `watch_all() doesn't include existing entries` → initial sync must handle missing timers
- VIOLATES I1: `fire_timer called with future fire_at` → `is_due()` check must guard firing
- VIOLATES I2: `KV delete before JetStream append` → write-ahead order enforced in fire_timer

## Ownership Contracts (Rust-specific)

- `js: &Context` - shared borrow, JetStream operations are thread-safe
- `timers: &Store` - shared borrow, KV operations are thread-safe
- `record: &TimerRecord` - shared borrow, read-only access
- `shutdown_rx: tokio::sync::watch::Receiver<bool>` - exclusive borrow during select

## Non-goals
- [NG1] Breaking changes to the `TimerFired` event format
- [NG2] Modifying the timer record serialization format
- [NG3] Changing the shutdown signal mechanism
