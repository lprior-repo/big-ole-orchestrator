# ADR 046: Async Process Supervisor Contract

## Status

Proposed

## Context

The `SpawnSupervisor` in `vo-actor` manages subprocess lifecycle for workflow instances. It currently has an implementation but lacks a formal contract defining types, invariants, error taxonomy, and behavioral guarantees. Without a formal contract:

1. Error handling semantics are ambiguous
2. State transitions are not formally specified
3. Recovery behavior under failures is undefined
4. The relationship between supervisor state and process lifecycle is unclear

This ADR defines the canonical runtime contract for the async process supervisor.

## Decision

### 1. Lifecycle State Machine

The supervised process follows this state machine:

```
┌─────────┐   spawn    ┌──────────────┐ health-check ┌─────────┐   healthy   ┌─────────┐
│  None   │ ─────────► │    Spawn     │ ───────────► │HealthChk│ ─────────► │ Running │
└─────────┘            └──────────────┘              └─────────┘            └─────────┘
                              │                            │                       │
                              │ failure                    │ failure               │ terminate
                              ▼                            ▼                       ▼
                        ┌─────────┐                  ┌─────────┐           ┌─────────┐
                        │  Failed │                  │  Failed │           │ Shutdown│
                        └─────────┘                  └─────────┘           └─────────┘
                              │                            │                       │
                              │ respawn                    │ respawn               │ exit
                              └────────────────────────────┴───────────────────────┘
                                                                                   │
                                                                                   ▼
                                                                           ┌─────────────┐
                                                                           │ Terminated  │
                                                                           └─────────────┘
```

#### SpawnPhase States

| State | Description | Valid Transitions |
|-------|-------------|-------------------|
| `Spawn` | Process is being started | `HealthCheck`, `Failed` |
| `HealthCheck` | Verifying process health | `Running`, `Failed` |
| `Running` | Process is healthy and running | `Shutdown` |
| `Shutdown` | Graceful termination in progress | `Terminated`, `Failed` |
| `Terminated` | Process has exited (terminal) | None |
| `Failed` | Process failed, may respawn | `Spawn` (respawn) |

#### Invariants

1. **Phase Atomicity**: A `SpawnRecord` is in exactly one phase at any time.
2. **Attempt Monotonicity**: `spawn_attempts` is monotonically increasing; it never decreases across transitions.
3. **Error Continuity**: `last_error` is `Some` if and only if the previous transition resulted in an error.
4. **PID Binding**: `spawn_id` (containing PID) is `Some` only in `Running` phase.

### 2. Error Taxonomy

All `SpawnSupervisorError` variants are classified as:

#### Transient (Retryable)
These errors may resolve on their own or with retries:

| Error | Trigger Condition |
|-------|-------------------|
| `StorageError` | Storage backend unavailable |
| `InstanceNotFound` | Instance actor restarting |
| `MailboxFull` | Instance mailbox at capacity |
| `DispatchError` | Message dispatch failed |

#### Resumable (May recover automatically)
These indicate process issues that may self-correct:

| Error | Trigger Condition |
|-------|-------------------|
| `HealthCheckFailed` | Process failed health check |
| `ProcessExited` | Process exited unexpectedly |
| `SpawnFailed` | Process spawn command failed |

#### Fatal (Requires intervention)
These indicate systemic issues:

| Error | Trigger Condition |
|-------|-------------------|
| `CorruptSpawn` | Spawn record malformed |
| `InvalidConfig` | Supervisor configuration invalid |
| `ZombieDetected` | Zombie process detected |

#### Operational (Expected lifecycle events)
These are normal operational states:

| Error | Trigger Condition |
|-------|-------------------|
| `AlreadyRunning` | Supervisor already started |
| `AlreadyShutdown` | Supervisor already stopped |
| `NotRunning` | Operation requires running supervisor |
| `ShutdownTimeout` | Graceful shutdown exceeded timeout |
| `AtomicityViolation` | Inconsistent state detected |

### 3. Supervisor State Machine

The `SpawnSupervisor` itself has independent state:

```
┌─────────┐   spawn()   ┌─────────┐   shutdown()   ┌─────────────┐   loop ends   ┌───────────┐
│ Stopped │ ──────────► │ Running │ ──────────────► │ ShuttingDown│ ────────────► │ ShutDown  │
└─────────┘             └─────────┘                 └─────────────┘               └───────────┘
```

#### Supervisor Invariants

1. **Single Active Supervisor**: Only one supervisor handle may be active per `SpawnSupervisor` instance.
2. **Clean Shutdown**: `shutdown()` must be called before dropping a supervisor to ensure graceful termination.
3. **State Broadcast**: All state transitions are broadcast via `watch::Sender<SupervisorState>`.

### 4. Behavioral Contract

#### 4.1 Spawn Contract

**Given** a valid `SpawnRecord` with phase `Spawn`:
- The supervisor SHALL attempt to spawn the process via `ProcessManager::spawn_process`
- On success, the supervisor SHALL transition the record to `HealthCheck`
- On failure, the supervisor SHALL store the error in `last_error` and leave phase as `Spawn`
- The supervisor SHALL increment `spawn_attempts` on each spawn attempt

**Postcondition**: After spawn attempt, record is either in `HealthCheck` (success) or `Spawn` with `last_error` set (failure).

#### 4.2 Health Check Contract

**Given** a `SpawnRecord` in `HealthCheck` phase:
- The supervisor SHALL perform up to `max_health_checks` health checks
- Health checks are spaced by `health_check_interval`
- If any check returns `false` (not healthy), the supervisor SHALL continue checking until max attempts
- If all checks pass, the supervisor SHALL transition to `Running`

**Postcondition**: After health checks complete, record is either `Running` (all checks passed) or `Failed` (checks exhausted).

#### 4.3 Respawn Contract

**Given** a `SpawnRecord` in `Failed` phase with `spawn_attempts < max_spawn_attempts`:
- The supervisor SHALL schedule a respawn with exponential backoff
- Backoff formula: `initial_backoff * backoff_multiplier^(attempt-1)` milliseconds
- After backoff expires, the supervisor SHALL call `respawn()` creating a new `SpawnRecord`

**Postcondition**: After backoff, a new `SpawnRecord` is created with `spawn_attempts` incremented.

#### 4.4 Shutdown Contract

**Given** a running supervisor:
- `shutdown()` SHALL send shutdown signal via `broadcast::Sender<()>`
- The supervisor SHALL transition to `ShuttingDown` state
- The supervisor SHALL complete any in-flight process cycles
- The supervisor SHALL transition to `ShutDown` when loop exits

**Postcondition**: Supervisor reaches `ShutDown` state; all spawned processes receive termination signal.

### 5. Storage Contract

The `SpawnStorage` trait provides these guarantees:

#### 5.1 Record Operations

| Operation | Guarantee |
|-----------|-----------|
| `get_spawn_record` | Returns `Some(record)` if exists, `None` otherwise |
| `save_spawn_record` | Atomically persists record; returns error on failure |
| `delete_spawn_record` | Removes record; returns error if not found |
| `scan_spawns_by_phase` | Returns up to `max` records in given phase, unordered |
| `transition_phase` | Atomically updates phase; returns error if record not found |

#### 5.2 Atomicity Guarantees

- **Phase Transition**: `transition_phase` is atomic; partial updates are not possible.
- **Spawn Dispatch**: The supervisor implements a saga pattern: storage update → dispatch. If dispatch fails, storage is already updated (acceptable since dispatch can be retried).

### 6. Process Manager Contract

The `ProcessManager` trait provides:

| Operation | Guarantee |
|-----------|-----------|
| `spawn_process` | Returns `ProcessHandle` with valid PID on success |
| `check_health` | Returns `Ok(true)` if healthy, `Ok(false)` if not, `Err` on system error |
| `is_zombie` | Returns `Ok(true)` if process is zombie |
| `terminate` | Sends SIGTERM; returns when process exits or timeout |
| `wait` | Waits for process exit; returns exit code |

### 7. Observability

The supervisor MUST emit telemetry for:

| Metric | Description |
|--------|-------------|
| `spawns_successful` | Counter of successful spawns |
| `spawns_failed` | Counter of failed spawns |
| `health_checks_performed` | Counter of health checks performed |
| `health_checks_failed` | Counter of failed health checks |
| `zombies_detected` | Counter of zombie detections |
| `respawns` | Counter of respawn events |
| `dispatch_errors` | Counter of dispatch errors |

### 8. Cancellation Safety

The supervisor loop is cancellation-safe:

- `shutdown()` waits for the loop to reach `ShutDown` state before returning
- In-flight `process_cycle()` operations complete before shutdown completes
- No state is lost if the supervisor task is cancelled

### 9. Send+Sync Requirements

All shared state (`SpawnSupervisor`, `SpawnSupervisorHandle`) MUST be `Send + Sync` to support multi-threaded tokio runtime.

## Consequences

### Positive

- Error handling becomes deterministic and testable
- State machine invariants can be verified via property-based tests
- Recovery behavior under failures is formally specified
- New implementations can verify compliance against contract

### Negative

- Contract may restrict future optimization opportunities
- Additional ceremony for implementing new `ProcessManager` or `SpawnStorage`

## References

- Implementation: `crates/vo-actor/src/spawn_supervisor.rs`
- Related ADR: ADR-041 (Managed Connector Runtime Contract)
- Related ADR: ADR-030 (Managed Effects and Sink Contracts)