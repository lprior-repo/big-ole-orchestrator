## Contract: Health Check Probe Framework

### 1. Purpose

Defines the contract for a reusable health check probe framework in the veloxide event-sourced actor system. This contract establishes types, invariants, and error taxonomy for probing the health of spawned processes and subsystems.

### 2. Source ADRs

- `docs/adr/v2/ADR-012-v2-execution-boundary-hardening.md` (execution boundary)
- `docs/adr/v2/ADR-015-v2-actor-invariants-backpressure.md` (actor health semantics)
- `docs/adr/v2/ADR-039-v2-hierarchical-lifecycle-state-machine.md` (lifecycle state machine)

### 3. Probe Types

#### 3.1 ProbeTarget

Identifies what is being probed.

```
ProbeTarget {
  target_type: ProbeTargetType,
  target_id: TargetId,
}
```

```
enum ProbeTargetType {
  Process,       // A spawned subprocess
  Actor,         // An actor instance
  Storage,       // A storage partition
  Connector,     // An external connector
  Custom(String), // Extension point for custom targets
}
```

#### 3.2 ProbeResult

Result of a single probe execution.

```
ProbeResult {
  target: ProbeTarget,
  outcome: ProbeOutcome,
  latency_ms: u64,
  timestamp: TimestampMs,
  details: Option<ProbeDetails>,
}
```

```
enum ProbeOutcome {
  Healthy,              // Target is healthy
  Unhealthy(String),     // Target is unhealthy with reason
  Unknown(String),       // Cannot determine health with reason
  Timeout,               // Probe timed out
}
```

#### 3.3 ProbeDetails

Additional context about the probe result.

```
ProbeDetails {
  probe_name: String,
  check_number: u32,
  attempt_number: u32,
  metadata: HashMap<String, String>,
}
```

#### 3.4 ProbeConfig

Configuration for probe behavior.

```
ProbeConfig {
  timeout: Duration,
  interval: Duration,
  max_retries: u32,
  retry_delay: Duration,
  healthy_threshold: u32,  // Consecutive successes to be healthy
  unhealthy_threshold: u32, // Consecutive failures to be unhealthy
}
```

#### 3.5 ProbeSchedule

Defines when probes execute.

```
ProbeSchedule {
  initial_delay: Duration,
  interval: Duration,
  backoff: Option<BackoffStrategy>,
}
```

```
enum BackoffStrategy {
  Fixed(Duration),
  Exponential { 
    initial: Duration,
    multiplier: f64,
    max: Duration,
  },
}
```

### 4. Probe States

#### 4.1 ProbeState

Runtime state of a probe.

```
ProbeState {
  target: ProbeTarget,
  current_status: HealthStatus,
  consecutive_healthy: u32,
  consecutive_unhealthy: u32,
  last_result: Option<ProbeResult>,
  last_check_time: Option<TimestampMs>,
  next_scheduled_check: Option<TimestampMs>,
}
```

#### 4.2 HealthStatus

Computed health status based on probe results.

```
enum HealthStatus {
  Unknown,           // No probes have run
  Pending,           // Probing in progress
  Healthy,           // Above healthy threshold
  Degraded,          // Some failures but below unhealthy threshold
  Unhealthy,         // Above unhealthy threshold
  Critical,          // Probe system itself is failing
}
```

### 5. Invariants (INV-*)

- **INV-001**: A target transitions to `Healthy` only after `healthy_threshold` consecutive successful probes
- **INV-002**: A target transitions to `Unhealthy` only after `unhealthy_threshold` consecutive failed probes
- **INV-003**: A probe must complete or timeout within the configured `timeout` duration
- **INV-004**: `consecutive_healthy` resets to 0 on any probe failure
- **INV-005**: `consecutive_unhealthy` resets to 0 on any probe success
- **INV-006**: `HealthStatus` transitions follow: `Unknown → Pending → {Healthy|Degraded|Unhealthy|Critical}`
- **INV-007**: A target in terminal state (`Completed`, `Failed`, `Cancelled`) cannot be probed
- **INV-008**: Probe scheduling respects `initial_delay` for the first probe
- **INV-009**: Backoff is applied only after a failure, not before the first probe
- **INV-010**: `last_result.timestamp` must be <= `next_scheduled_check` when a probe is scheduled

### 6. Error Taxonomy

```rust
struct ProbeError {
    category: ProbeErrorCategory,
    detail: ProbeErrorDetail,
    context: ProbeContext,
}

enum ProbeErrorCategory {
    ProbeSystemFailure,     // The probe subsystem itself failed
    TargetUnreachable,      // Cannot reach the target
    TargetRespondedError,   // Target responded but with error
    Timeout,                // Probe timed out
    ConfigurationError,     // Invalid probe configuration
    ResourceExhaustion,     // Cannot allocate probe resources
}

enum ProbeErrorDetail {
    ProcessNotFound(u32),           // PID does not exist
    ActorNotFound(InstanceId),      // Actor instance not found
    StoragePartitionMissing(String), // Storage partition not found
    ConnectionFailed(String),       // Cannot connect to target
    MalformedResponse(String),      // Target response was malformed
    ProbeItselfFailed(String),     // Probe execution itself failed
    QueueFull,                     // Probe queue is full
    ScheduleOverflow,              // Next scheduled time overflowed
}

struct ProbeContext {
    target: ProbeTarget,
    probe_name: String,
    attempt: u32,
    timestamp: TimestampMs,
}
```

### 7. Probe Protocol

1. **Schedule**: Determine next probe time based on `ProbeSchedule`
2. **Execute**: Run probe against target with configured `timeout`
3. **Record**: Store result in `ProbeResult` with `latency_ms` and `timestamp`
4. **Update State**: Update `ProbeState.consecutive_healthy/unhealthy` counters
5. **Compute Status**: Derive `HealthStatus` from consecutive counters and thresholds
6. **Notify**: Emit notification if `HealthStatus` changed

### 8. Constraints

- Probes must be idempotent; multiple probes must not cause side effects
- A probe must not block other probes; concurrent probe execution is allowed
- The probe subsystem must continue operating even if individual probes fail
- Probe results must be stored durably for debugging and audit
- A probe that times out counts as a failure for `consecutive_unhealthy`

### 9. Relevant Files

- `crates/vo-actor/src/spawn_supervisor.rs` (existing health check implementation)
- `crates/vo-types/src/instance_status.rs` (status types)
- `crates/vo-types/src/lifecycle_superstate.rs` (lifecycle states)
- `crates/vo-types/src/integer_types.rs` (TimestampMs, Duration types)

### 10. Acceptance Criteria

- Probe types compile and cover all target types in the system
- Probe outcomes are exhaustive for all observed probe scenarios
- All invariants (INV-001 through INV-010) are formally stated
- Error taxonomy covers probe system failures, target failures, and configuration errors
- The contract is self-contained and does not reference nonexistent crates or files
- Probe state transitions are deterministic based on probe results and thresholds