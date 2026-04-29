## Contract: Connection Pool Manager

### 1. Purpose

Defines the contract for managing NATS client connections in the veloxide distributed worker system. This contract establishes the types, invariants, and error taxonomy for the connection pool subsystem that manages lifecycle, multiplexing, and resilience of NATS connections used by `vo-worker` for distributed locking and messaging.

### 2. Source ADRs

- `docs/adr/v2/ADR-013-v2-system-resilience.md` (connection resilience baseline)
- `docs/adr/v2/ADR-014-v2-secure-ipc-fd-management.md` (resource management)
- `docs/adr/v2/ADR-026-v2-ai-loop-poisoning-circuit-breakers.md` (circuit breaker patterns)

### 3. Connection Pool Types

#### 3.1 PoolConfig

Configuration for the connection pool.

```
PoolConfig {
  min_connections: u32,           // Minimum idle connections to maintain
  max_connections: u32,           // Maximum total connections allowed
  connection_timeout_ms: u64,     // Timeout for acquiring a connection
  idle_timeout_ms: u64,           // Max idle time before connection is closed
  health_check_interval_ms: u64,  // Interval for health-checking idle connections
  max_pending_acquires: u32,     // Max requests waiting for a connection
}
```

#### 3.2 PooledConnection

Represents a connection in the pool with metadata.

```
PooledConnection {
  connection_id: ConnectionId,
  created_at: TimestampMs,
  last_used_at: TimestampMs,
  use_count: u64,                // Number of times this connection has been used
  status: ConnectionStatus,
  nats_connection: NatsConnection, // The underlying NATS connection
}
```

#### 3.3 ConnectionId

Unique identifier for a pooled connection.

```
ConnectionId(String) // ULID-based unique identifier
```

#### 3.4 ConnectionStatus Enum

```
enum ConnectionStatus {
  Idle,              // Available for checkout
 CheckedOut,         // Currently in use by a caller
  HealthCheck,       // Being health-checked
  Closing,           // Graceful shutdown in progress
  Closed,            // Fully terminated
}
```

#### 3.5 AcquireResult

Result of attempting to acquire a connection from the pool.

```
enum AcquireResult {
  Available { connection: PooledConnection },
  Pending { wait_handle: WaitHandle },
  PoolExhausted { config: PoolConfig },
  PoolClosing,
  Timeout { waited_ms: u64 },
}
```

#### 3.6 WaitHandle

Handle for a pending acquire request.

```
WaitHandle {
  request_id: RequestId,
  enqueued_at: TimestampMs,
  pool_id: PoolId,
}
```

#### 3.7 PoolId

Identifies a specific connection pool instance.

```
PoolId(String) // Typically corresponds to a NATS server subject namespace
```

#### 3.8 HealthCheckResult

Result of a connection health check.

```
enum HealthCheckResult {
  Healthy,
  Stale,                 // Connection is dead but gracefully closeable
  Corrupted,             // Connection state is inconsistent
  Timeout,
}
```

### 4. Pool Operations

#### 4.1 Core Operations

```rust
trait ConnectionPool {
    // Initialize the pool with configuration
    fn new(config: PoolConfig, nats_urls: Vec<String>) -> Self;

    // Acquire a connection from the pool (blocking or async)
    async fn acquire(&self) -> AcquireResult;

    // Release a connection back to the pool
    async fn release(&self, connection: PooledConnection) -> ReleaseResult;

    // Forcefully evict a connection (e.g., after health check failure)
    async fn evict(&self, connection_id: ConnectionId) -> EvictResult;

    // Get current pool statistics
    fn stats(&self) -> PoolStats;

    // Gracefully shutdown the pool
    async fn shutdown(&self) -> ShutdownResult;
}
```

#### 4.2 ReleaseResult Enum

```
enum ReleaseResult {
    Returned,
    AlreadyClosed,
    Evicted { reason: EvictionReason },
}
```

#### 4.3 EvictionReason Enum

```
enum EvictionReason {
    HealthCheckFailed(HealthCheckResult),
    ExplicitEviction,
    IdleTimeout,
    ProtocolError(String),
}
```

#### 4.4 PoolStats

Current state statistics for the pool.

```
struct PoolStats {
    pool_id: PoolId,
    total_connections: u32,
    idle_connections: u32,
    checked_out_connections: u32,
    pending_acquires: u32,
    total_acquires: u64,
    total_releases: u64,
    total_evictions: u64,
    total_health_checks: u64,
    failed_health_checks: u64,
}
```

### 5. Invariants (INV-*)

- **INV-001**: `min_connections <= max_connections` at all times; pool never violates this constraint
- **INV-002**: `checked_out_connections + idle_connections + pending_acquires <= max_connections + max_pending_acquires`
- **INV-003**: A connection in `Idle` state is always safe to checkout; health checks ensure this
- **INV-004**: `use_count` monotonically increases; never resets during a connection's lifetime
- **INV-005**: When `idle_timeout_ms` elapses on an idle connection, the connection is closed and removed from the pool
- **INV-006**: `connection_timeout_ms` bounds all acquire operations; no acquire blocks indefinitely
- **INV-007**: During `shutdown`, no new connections are issued; existing connections are gracefully closed
- **INV-008**: A connection that fails health check is never returned to `Idle` state; it is evicted
- **INV-009**: Circuit breaker trips when `failed_health_checks > max_connections * 0.5` within a sliding window
- **INV-010**: Pool statistics are eventually consistent; they reflect actual state within one health-check cycle

### 6. Error Taxonomy

```rust
struct ConnectionPoolError {
    category: ErrorCategory,
    detail: ErrorDetail,
    context: ErrorContext,
}

enum ErrorCategory {
    PoolExhaustion,       // Cannot create connections, at capacity
    Timeout,              // Operation timed out waiting for resources
    ConnectionFailed,     // Underlying NATS connection failed
    HealthCheckFailed,    // Connection failed health validation
    InvalidState,         // Operation invalid for current state
    ShutdownInProgress,   // Pool is closing, operation rejected
    ResourceExhaustion,  // Cannot allocate internal resources
}

enum ErrorDetail {
    MaxConnectionsReached { max: u32 },
    PendingAcquiresExceeded { max: u32 },
    AcquireTimeout { waited_ms: u64, timeout_ms: u64 },
    NatsConnectionError { connection_id: ConnectionId, reason: String },
    HealthCheckTimeout { connection_id: ConnectionId },
    ConnectionCorrupted { connection_id: ConnectionId },
    InvalidRelease { reason: &'static str },
    PoolNotInitialized,
    AlreadyShutdown,
    CircuitBreakerOpen { consecutive_failures: u32 },
}

struct ErrorContext {
    pool_id: PoolId,
    timestamp: TimestampMs,
    operation: &'static str,
    connection_id: Option<ConnectionId>,
}
```

### 7. Circuit Breaker Integration

#### 7.1 CircuitBreakerState Enum

```
enum CircuitBreakerState {
    Closed,     // Normal operation, tracking failures
    Open,       // Rejecting all acquire requests
    HalfOpen,   // Testing if connections can recover
}
```

#### 7.2 Circuit Breaker Rules

- **CB-001**: Transitions `Closed -> Open` when failure rate exceeds 50% in a 30-second window
- **CB-002**: `Open` state auto-transitions to `HalfOpen` after `connection_timeout_ms`
- **CB-003**: `HalfOpen` allows `max_connections` test acquisitions; success transitions to `Closed`
- **CB-004**: `HalfOpen` failure count >= `max_connections` transitions back to `Open`
- **CB-005**: While `Open`, all `acquire()` calls return `PoolExhausted` with `CircuitBreakerOpen` detail

### 8. Connection Lifecycle

1. **Create**: Pool creates connection on startup up to `min_connections`
2. **Checkout**: Caller acquires via `acquire()`, connection moves `Idle -> CheckedOut`
3. **Use**: Caller uses connection for NATS operations
4. **Return**: Caller releases via `release()`, connection moves `CheckedOut -> Idle`
5. **Health Check**: Periodic background task checks idle connections
6. **Evict**: Failed health check or explicit eviction removes connection
7. **Close**: Graceful shutdown closes all connections

### 9. Constraints

- A pooled connection must not be shared across concurrent callers; ownership is exclusive
- The pool must not block the tokio runtime thread during connection creation
- All timeouts must be configurable; defaults must be sane for production use
- Health checks must not interfere with active connections; only idle connections are checked
- The circuit breaker must be thread-safe and observable via metrics

### 10. Relevant Files

- `crates/vo-worker/src/lib.rs` (distributed lock manager using NATS, candidate for pool integration)
- `crates/vo-types/src/integer_types.rs` (TimestampMs, TimeoutMs type patterns)
- `crates/vo-types/src/errors.rs` (error taxonomy pattern)
- `crates/vo-types/src/connector/mod.rs` (connector state machine patterns)

### 11. Acceptance Criteria

- All types compile and cover the complete connection lifecycle states
- Invariants (INV-001 through INV-010) are formally stated and testable
- Error taxonomy covers acquisition, release, health check, and circuit breaker failure modes
- The contract is self-contained and does not reference nonexistent crates or files
- Circuit breaker integration is explicitly defined with state transitions
- Pool statistics are sufficient for observability and alerting