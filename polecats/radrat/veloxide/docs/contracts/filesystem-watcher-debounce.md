## Contract: Filesystem Watcher with Debounce

### 1. Purpose

Defines the contract for a debounced filesystem event watcher in the veloxide event-sourced actor system. This contract establishes types, invariants, and error taxonomy for coalescing rapid filesystem events into stable, debounced notifications.

### 2. Source ADRs

- `docs/adr/v2/ADR-012-v2-execution-boundary-hardening.md` (execution boundary)
- `docs/adr/v2/ADR-015-v2-actor-invariants-backpressure.md` (actor health semantics)
- `docs/adr/v2/ADR-039-v2-hierarchical-lifecycle-state-machine.md` (lifecycle state machine)

### 3. Event Types

#### 3.1 FileEvent

Represents a filesystem change event.

```
FileEvent {
  variant: FileEventVariant,
  path: PathBuf,
}
```

```
enum FileEventVariant {
  Modify,  // File was modified
  Delete,  // File was deleted
}
```

#### 3.2 Debouncer

The central debouncing state machine. Accepts raw FileEvents and yields stabilized events after debounce duration elapses.

```
Debouncer {
  duration: Duration,          // Debounce window
  ready_rx: Receiver<Result<PathBuf, Error>>,  // Channel yielding debounced paths
}
```

### 4. Invariants (INV-*)

- **INV-001**: A `Debouncer` instance is created with a strictly positive `duration`; zero duration returns `Error::InvalidDebounceDuration`
- **INV-002**: A `Debouncer` requires an active Tokio runtime; creation outside a runtime returns `Error::NoRuntime`
- **INV-003**: The `event_rx` channel must be connected at creation; a disconnected channel returns `Error::WatcherChannelClosed`
- **INV-004**: `Modify` events for a path reset (or establish) that path's pending deadline
- **INV-005**: `Delete` events immediately remove the path from the pending set, canceling any pending debounce
- **INV-006**: A path is yielded exactly once after its deadline expires, then removed from pending
- **INV-007**: Multiple `Modify` events for the same path within the debounce window collapse into a single yield
- **INV-008**: When `event_rx` is closed and all pending events are drained, `Error::WatcherChannelClosed` is sent before the channel closes
- **INV-009**: The background task runs until `event_rx` is closed AND `pending` is empty, preventing premature termination
- **INV-010**: `Instant::checked_add` is used for deadline computation; overflow returns `Error::DebouncerInternal`
- **INV-011**: The background task does not panic; errors are propagated via the `ready_tx` channel
- **INV-012**: Yields are emitted in sorted order by `PathBuf` to ensure deterministic output

### 5. Error Taxonomy

```rust
enum Error {
  InvalidDebounceDuration,   // Duration was zero
  WatcherChannelClosed,     // Event receiver dropped or all events drained
  DebouncerInternal,        // Timer overflow or internal failure
  NoRuntime,                // No Tokio runtime available
}
```

#### 5.1 Error Categories

| Variant | Category | Description |
|---------|----------|-------------|
| `InvalidDebounceDuration` | Configuration | Caller provided zero duration |
| `WatcherChannelClosed` | InputEOF | Event source exhausted |
| `DebouncerInternal` | System | Timer overflow or internal invariant violation |
| `NoRuntime` | Runtime | Tokio runtime not available |

### 6. Debouncer Protocol

1. **Create**: Validate `duration > 0`, runtime presence, and channel connectivity
2. **Spawn**: Launch background task with initial event (if any)
3. **Coalesce**: Accumulate `Modify` events, resetting deadlines per path
4. **Cancel**: Remove paths on `Delete` events
5. **Emit**: When deadline expires, send path via `ready_rx`
6. **Terminate**: Close `ready_rx` when source exhausted and pending empty

### 7. Constraints

- The debouncer must not drop events; every `Modify` must eventually yield exactly once if not deleted
- The debouncer must be `Send + Sync` to support actor integration
- The background task must not block; event processing must be cooperative
- Deadline computation must not panic; overflow is fatal and returns `Error::DebouncerInternal`
- The debouncer does not buffer infinitely; bounded channel prevents memory exhaustion
- Yields are deterministic and sorted to support reproducible testing

### 8. Relevant Files

- `crates/vo-core/src/debounce.rs` (implementation)
- `crates/vo-core/proptest-regressions/debounce.txt` (property test regression)

### 9. Acceptance Criteria

- `FileEvent` enum covers all filesystem event types observed by the watcher
- `Error` enum is exhaustive and covers all failure modes
- All invariants (INV-001 through INV-012) are formally stated
- The contract is self-contained and does not reference nonexistent crates or files
- Debouncer behavior is deterministic: same event sequence yields same output sequence
- Delete events cancel pending Modify debounce for the same path
- The contract supports both unit testing and formal verification (Kani proofs present)