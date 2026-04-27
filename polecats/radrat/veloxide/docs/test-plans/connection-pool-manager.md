# Test Plan: Connection Pool Manager

**Contract**: `docs/contracts/connection-pool-manager.md`
**Issue**: ve-kai3
**Target crate**: `crates/vo-worker/src/` (pool implementation + tests)

## Scope

This plan covers exhaustive testing for the `ConnectionPool`, its types, all 10 invariants (INV-001 through INV-010), the full error taxonomy, circuit breaker integration (CB-001 through CB-005), connection lifecycle, edge cases, and property-based invariants. Tests are organized by the Testing Trophy: unit tests (majority), targeted property tests via proptest, and targeted integration tests for multi-operation sequences.

---

## 1. Type Construction & Validation Tests

### 1.1 PoolConfig

| ID | Test | Category | Expected |
|----|------|----------|----------|
| PC-001 | Valid config with min=1, max=10 | Happy path | Ok |
| PC-002 | min_connections == max_connections | Boundary | Ok |
| PC-003 | min_connections > max_connections | Validation (INV-001) | Err(InvalidConfig) |
| PC-004 | max_connections == 0 | Validation | Err(InvalidConfig) |
| PC-005 | connection_timeout_ms == 0 | Validation | Err(InvalidConfig) |
| PC-006 | idle_timeout_ms == 0 | Validation | Err(InvalidConfig) |
| PC-007 | health_check_interval_ms == 0 | Validation | Err(InvalidConfig) |
| PC-008 | max_pending_acquires == 0 | Validation | Ok (0 = no pending queue) |
| PC-009 | All fields at maximum u32/u64 values | Boundary | Ok |
| PC-010 | Sane defaults constructor | Happy path | Ok with production-ready values |

### 1.2 ConnectionId

| ID | Test | Category | Expected |
|----|------|----------|----------|
| CI-001 | Generate unique IDs in sequence | Happy path | All distinct |
| CI-002 | IDs are time-ordered (ULID property) | Property | Monotonically sortable |
| CI-003 | Serde round-trip preserves value | Correctness | Eq |
| CI-004 | Display format is valid | Correctness | Parseable string |
| CI-005 | Two pools produce distinct ConnectionIds | Isolation | No collisions |

### 1.3 PoolId

| ID | Test | Category | Expected |
|----|------|----------|----------|
| PI-001 | Construct from string | Happy path | Ok |
| PI-002 | Equality works correctly | Correctness | Same string -> Eq |
| PI-003 | Hash consistency | Correctness | Same string -> same hash |
| PI-004 | Display format matches input | Correctness | to_string() == original |

### 1.4 ConnectionStatus

| ID | Test | Category | Expected |
|----|------|----------|----------|
| CS-001 | All variants are distinct | Correctness | 5 distinct variants |
| CS-002 | Debug/Display format is readable | Correctness | Contains variant name |
| CS-003 | Serde round-trip preserves variant | Correctness | Eq |

### 1.5 AcquireResult

| ID | Test | Category | Expected |
|----|------|----------|----------|
| AR-001 | Available variant holds connection | Happy path | connection field populated |
| AR-002 | Pending variant holds wait_handle | Happy path | wait_handle field populated |
| AR-003 | PoolExhausted variant holds config | Exhaustion | config field populated |
| AR-004 | PoolClosing variant is unit | State | No fields |
| AR-005 | Timeout variant holds waited_ms | Timeout | waited_ms > 0 |

### 1.6 HealthCheckResult

| ID | Test | Category | Expected |
|----|------|----------|----------|
| HR-001 | Healthy variant is unit | Happy path | No fields |
| HR-002 | Stale variant is unit | Degraded | No fields |
| HR-003 | Corrupted variant is unit | Error | No fields |
| HR-004 | Timeout variant is unit | Timeout | No fields |
| HR-005 | All variants are distinct | Correctness | 4 distinct variants |

### 1.7 WaitHandle

| ID | Test | Category | Expected |
|----|------|----------|----------|
| WH-001 | Construct with request_id, enqueued_at, pool_id | Happy path | Ok |
| WH-002 | enqueued_at is captured at creation | Correctness | Matches approximate time |
| WH-003 | Serde round-trip preserves all fields | Correctness | Eq |

---

## 2. Pool Lifecycle Tests

### 2.1 Initialization

| ID | Test | Category | Expected |
|----|------|----------|----------|
| PL-001 | New pool with min=0 starts with 0 connections | Happy path | stats().total_connections == 0 |
| PL-002 | New pool with min=3 creates 3 idle connections | Happy path | stats().idle_connections == 3 |
| PL-003 | New pool with min > max is rejected | INV-001 | Err(InvalidConfig) |
| PL-004 | New pool stats show zero acquires/releases/evictions | Happy path | All counters == 0 |
| PL-005 | New pool circuit breaker is in Closed state | Happy path | CB state == Closed |
| PL-006 | PoolId matches configured subject namespace | Correctness | Correct PoolId |
| PL-007 | Pool initializes without blocking tokio runtime | Async safety | Completes without blocking |

### 2.2 Acquire

| ID | Test | Category | Expected |
|----|------|----------|----------|
| AQ-001 | Acquire from pool with idle connection returns Available | Happy path | AcquireResult::Available |
| AQ-002 | Acquire moves connection Idle -> CheckedOut | State | Connection status updated |
| AQ-003 | Acquire increments stats.total_acquires | Stats | Count += 1 |
| AQ-004 | Acquire decrements stats.idle_connections | Stats | idle -= 1 |
| AQ-005 | Acquire increments stats.checked_out_connections | Stats | checked_out += 1 |
| AQ-006 | Acquire sets connection.last_used_at to now | Lifecycle | Timestamp updated |
| AQ-007 | Acquire increments connection.use_count | INV-004 | use_count += 1 |
| AQ-008 | Acquire exclusive ownership (no sharing) | Constraint | Two acquires return different connections |
| AQ-009 | Acquire with all connections checked out returns Pending | Capacity | AcquireResult::Pending |
| AQ-010 | Acquire respects max_pending_acquires limit | Capacity | PoolExhausted when exceeded |
| AQ-011 | Acquire during shutdown returns PoolClosing | State | AcquireResult::PoolClosing |
| AQ-012 | Acquire with circuit breaker Open returns PoolExhausted | CB-005 | PoolExhausted with CircuitBreakerOpen |
| AQ-013 | Acquire respects connection_timeout_ms | INV-006 | AcquireResult::Timeout after timeout |
| AQ-014 | Acquire triggers new connection creation when pool below max | Scaling | New connection created |
| AQ-015 | Acquire does NOT create connection when at max_connections | INV-002 | Returns Pending or PoolExhausted |
| AQ-016 | Acquire with min=0 and no connections creates on demand | Lazy init | Available with new connection |
| AQ-017 | Pending acquire is fulfilled when connection released | Queue | WaitHandle resolved |

### 2.3 Release

| ID | Test | Category | Expected |
|----|------|----------|----------|
| RL-001 | Release moves connection CheckedOut -> Idle | State | Connection status updated |
| RL-002 | Release returns Returned | Happy path | ReleaseResult::Returned |
| RL-003 | Release increments stats.total_releases | Stats | Count += 1 |
| RL-004 | Release decrements stats.checked_out_connections | Stats | checked_out -= 1 |
| RL-005 | Release increments stats.idle_connections | Stats | idle += 1 |
| RL-006 | Release connection from different pool is rejected | Isolation | Err(InvalidRelease) |
| RL-007 | Release already-closed connection returns AlreadyClosed | State | ReleaseResult::AlreadyClosed |
| RL-008 | Release connection that exceeds max idle triggers eviction | INV-005 | ReleaseResult::Evicted(IdleTimeout) |
| RL-009 | Release during shutdown closes connection | Lifecycle | ReleaseResult::Evicted |
| RL-010 | Release triggers pending acquire fulfillment | Queue | Next waiter gets connection |

### 2.4 Evict

| ID | Test | Category | Expected |
|----|------|----------|----------|
| EV-001 | Evict idle connection removes it from pool | Happy path | Connection removed |
| EV-002 | Evict increments stats.total_evictions | Stats | Count += 1 |
| EV-003 | Evict non-existent connection is idempotent | Edge case | Ok, no error |
| EV-004 | Evict checked-out connection fails or queues deferred eviction | State | Err or deferred |
| EV-005 | Evict decrements total_connections | Stats | total -= 1 |
| EV-006 | Evict triggers replacement if below min_connections | INV-001 | New connection created |

### 2.5 Shutdown

| ID | Test | Category | Expected |
|----|------|----------|----------|
| SD-001 | Shutdown closes all idle connections | Happy path | idle_connections == 0 |
| SD-002 | Shutdown waits for checked-out connections to be released | INV-007 | Graceful drain |
| SD-003 | Shutdown rejects new acquire requests | INV-007 | AcquireResult::PoolClosing |
| SD-004 | Shutdown with timeout force-closes remaining connections | Lifecycle | All connections closed |
| SD-005 | Shutdown is idempotent (calling twice is safe) | Safety | No panic or error |
| SD-006 | Shutdown transitions circuit breaker to Closed | Lifecycle | CB state consistent |
| SD-007 | Post-shutdown stats reflect final state | Stats | Accurate final counts |

---

## 3. Circuit Breaker Tests

### 3.1 State Transitions

| ID | Test | Category | Expected |
|----|------|----------|----------|
| CB-001 | Closed -> Open when failure rate > 50% in 30s window | CB-001 | State transitions |
| CB-002 | Closed -> Open requires > 50% not == 50% | CB-001 | Stays Closed at exactly 50% |
| CB-003 | Open -> HalfOpen after connection_timeout_ms | CB-002 | Auto-transition |
| CB-004 | HalfOpen allows test acquisitions up to max_connections | CB-003 | Acquires succeed |
| CB-005 | HalfOpen success transitions to Closed | CB-003 | State transitions |
| CB-006 | HalfOpen failure count >= max_connections transitions to Open | CB-004 | Back to Open |
| CB-007 | Open state rejects all acquire() with PoolExhausted | CB-005 | CircuitBreakerOpen detail |
| CB-008 | Failure window resets after transition to Closed | Lifecycle | Fresh tracking |
| CB-009 | Sliding window only considers last 30 seconds | Temporal | Old failures excluded |

### 3.2 Circuit Breaker Integration

| ID | Test | Category | Expected |
|----|------|----------|----------|
| CI-001 | Health check failures increment circuit breaker failure count | INV-009 | Count updated |
| CI-002 | Successful health checks do NOT increment failure count | INV-009 | Count unchanged |
| CI-003 | Circuit breaker is thread-safe under concurrent access | Concurrency | No data races |
| CI-004 | Circuit breaker state is observable via stats | Observability | State accessible |
| CI-005 | Circuit breaker trips when failed_health_checks > max * 0.5 | INV-009 | Trips correctly |
| CI-006 | Circuit breaker does not trip below threshold | INV-009 | Remains Closed |

---

## 4. Health Check Tests

### 4.1 Health Check Execution

| ID | Test | Category | Expected |
|----|------|----------|----------|
| HC-001 | Healthy connection returns HealthCheckResult::Healthy | Happy path | Healthy |
| HC-002 | Stale (dead but closeable) connection returns Stale | Degraded | Stale |
| HC-003 | Corrupted connection returns Corrupted | Error | Corrupted |
| HC-004 | Health check timeout returns Timeout | Timeout | Timeout |
| HC-005 | Health check increments stats.total_health_checks | Stats | Count += 1 |
| HC-006 | Failed health check increments stats.failed_health_checks | Stats | failed += 1 |
| HC-007 | Health check only runs on idle connections | Constraint | CheckedOut connections skipped |
| HC-008 | Health check runs at configured interval | Timing | Respects health_check_interval_ms |
| HC-009 | Failed health check triggers eviction | INV-008 | Connection evicted |
| HC-010 | Failed health check never returns connection to Idle | INV-008 | Status != Idle |
| HC-011 | Health check on pool with 0 idle connections is no-op | Edge case | No errors |

### 4.2 Idle Timeout

| ID | Test | Category | Expected |
|----|------|----------|----------|
| IT-001 | Connection idle beyond idle_timeout_ms is closed | INV-005 | Connection removed |
| IT-002 | Connection idle just under timeout is preserved | Boundary | Connection remains |
| IT-003 | Idle timeout eviction decrements idle_connections | Stats | idle -= 1 |
| IT-004 | Idle timeout triggers replacement if below min_connections | INV-001 | New connection created |
| IT-005 | Idle timeout is checked during acquire/release | Timing | Stale connections cleaned |

---

## 5. Pool Statistics Tests

### 5.1 Stats Accuracy

| ID | Test | Category | Expected |
|----|------|----------|----------|
| PS-001 | Initial stats are all zeros except total/idle (from min_connections) | Initial | Correct initial state |
| PS-002 | stats() after acquire shows updated counts | Accuracy | idle-1, checked_out+1, acquires+1 |
| PS-003 | stats() after release shows updated counts | Accuracy | idle+1, checked_out-1, releases+1 |
| PS-004 | stats() after evict shows updated counts | Accuracy | total-1, evictions+1 |
| PS-005 | stats() after shutdown reflects final state | INV-010 | Consistent with actual state |
| PS-006 | Stats are eventually consistent within one health-check cycle | INV-010 | Converge |
| PS-007 | PoolId in stats matches pool's PoolId | Correctness | Same PoolId |
| PS-008 | Total connections = idle + checked_out + health_check + closing | INV-002 | Count invariant holds |

---

## 6. Invariant Verification Tests

These tests explicitly verify each invariant holds after specific operations.

| ID | Invariant | Test Strategy |
|----|-----------|---------------|
| IV-001 | INV-001 | Construct PoolConfig with min > max; verify rejection. After eviction drops below min, verify new connection created. |
| IV-002 | INV-002 | After every operation (acquire/release/evict), verify checked_out + idle + pending <= max + max_pending. |
| IV-003 | INV-003 | Mark connection as Idle via release; immediately acquire; verify connection is usable (no state corruption). |
| IV-004 | INV-004 | Acquire same connection 100 times (release between each); verify use_count monotonically increases, never resets. |
| IV-005 | INV-005 | Create connection, leave idle, advance mock clock past idle_timeout_ms; verify connection is closed and removed. |
| IV-006 | INV-006 | Set connection_timeout_ms to 1ms, fill pool; verify acquire returns Timeout within bounded time, not indefinite block. |
| IV-007 | INV-007 | Call shutdown(); verify all subsequent acquire() calls return PoolClosing; verify no new connections created. |
| IV-008 | INV-008 | Force health check to fail; verify connection is evicted, never returned to Idle state. |
| IV-009 | INV-009 | Fail health checks on > 50% of connections in 30s window; verify circuit breaker trips to Open. |
| IV-010 | INV-010 | Perform burst of operations; verify stats reflect actual state within one health-check cycle. |

---

## 7. Error Taxonomy Tests

Each error variant must be produced by at least one test.

| ID | Error Category | Error Detail | Trigger |
|----|----------------|--------------|---------|
| ET-001 | PoolExhaustion | MaxConnectionsReached | Fill pool to max, attempt acquire |
| ET-002 | PoolExhaustion | PendingAcquiresExceeded | Fill pool + pending queue, attempt acquire |
| ET-003 | Timeout | AcquireTimeout | Set connection_timeout_ms=1, block all connections |
| ET-004 | ConnectionFailed | NatsConnectionError | Mock NATS connection failure on create |
| ET-005 | HealthCheckFailed | HealthCheckTimeout | Mock health check timeout |
| ET-006 | HealthCheckFailed | ConnectionCorrupted | Mock corrupted connection state |
| ET-007 | InvalidState | InvalidRelease | Release connection not from this pool |
| ET-008 | InvalidState | PoolNotInitialized | Operate on uninitialized pool |
| ET-009 | ShutdownInProgress | AlreadyShutdown | Acquire after shutdown |
| ET-010 | ResourceExhaustion | (internal) | Exhaust internal allocation limits |
| ET-011 | PoolExhaustion | CircuitBreakerOpen | Trip circuit breaker, attempt acquire |
| ET-012 | Timeout | (acquire timeout) | Acquire with all connections checked out, timeout expires |
| ET-013 | ConnectionFailed | (NATS error) | Mock NATS disconnect during active use |

---

## 8. Connection Lifecycle Tests

### 8.1 Full Lifecycle Sequence

| ID | Test | Category | Expected |
|----|------|----------|----------|
| LC-001 | Create -> Checkout -> Use -> Return -> Idle | Full lifecycle | All transitions correct |
| LC-002 | Create -> Checkout -> Health check skipped (CheckedOut) | Lifecycle | Health check does not interfere |
| LC-003 | Create -> Checkout -> Return -> Idle timeout -> Close | Full lifecycle | Connection removed after timeout |
| LC-004 | Create -> Checkout -> Return -> Health check fail -> Evict | Full lifecycle | Connection evicted, not returned to Idle |
| LC-005 | Create -> Checkout -> Return -> Shutdown | Full lifecycle | Connection closed during shutdown |
| LC-006 | Create -> Checkout -> Release -> Re-acquire same connection | Reuse | Same connection returned if available |
| LC-007 | Create -> Checkout -> Release -> Connection reused use_count=2 | INV-004 | use_count increments on re-acquire |

### 8.2 Concurrent Lifecycle

| ID | Test | Category | Expected |
|----|------|----------|----------|
| CL-001 | Concurrent acquires from multiple tasks | Concurrency | All get distinct connections |
| CL-002 | Concurrent releases from multiple tasks | Concurrency | All connections returned correctly |
| CL-003 | Concurrent acquire + release interleaving | Concurrency | Pool state remains consistent |
| CL-004 | Concurrent shutdown + acquire | Concurrency | Acquire gets PoolClosing |
| CL-005 | Concurrent evict + acquire | Concurrency | No data races, pool consistent |
| CL-006 | Concurrent health checks + acquires | Concurrency | Only idle connections checked |

---

## 9. Property-Based Tests (proptest)

| ID | Property | Strategy |
|----|----------|----------|
| PP-001 | **PoolConfig validation** | Arbitrary min/max/timeout values; verify valid configs pass, invalid fail |
| PP-002 | **Acquire-release preserves INV-002** | Arbitrary sequence of acquire/release; verify count invariant after each op |
| PP-003 | **use_count monotonicity** | Arbitrary acquire/release sequence on same connection; verify use_count never decreases |
| PP-004 | **Stats consistency** | Arbitrary operation sequence; verify stats.total == idle + checked_out + health_check + closing |
| PP-005 | **ConnectionId uniqueness** | Generate N ConnectionIds; verify all distinct |
| PP-006 | **Idle timeout correctness** | Arbitrary idle durations; verify connections closed iff duration > idle_timeout_ms |
| PP-007 | **Circuit breaker threshold** | Arbitrary failure sequences; verify trips exactly when rate > 50% in window |
| PP-008 | **Shutdown rejection** | After shutdown, arbitrary operations all fail gracefully |
| PP-009 | **Eviction replacement** | Evict when below min_connections; verify new connection created |
| PP-010 | **Acquire timeout bounded** | Arbitrary timeout values; verify acquire completes within timeout + epsilon |

---

## 10. Multi-Operation Sequence Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| SO-001 | Acquire all connections, release all, re-acquire all | Sequence | Pool returns to same state |
| SO-002 | Acquire, evict, acquire replacement | Sequence | New connection created after eviction |
| SO-003 | Fill pool, queue pending acquires, release one | Sequence | First pending waiter fulfilled |
| SO-004 | Shutdown while connections checked out, then release | Sequence | Graceful drain completes |
| SO-005 | Trip circuit breaker, wait for HalfOpen, succeed, verify Closed | Sequence | Full CB lifecycle |
| SO-006 | Health check fails all connections, verify all evicted | Sequence | Pool drains to 0 |
| SO-007 | Rapid acquire/release cycle (100 iterations) | Stress | Pool state consistent throughout |
| SO-008 | Acquire, idle timeout expires, acquire replacement | Sequence | Old connection closed, new one provided |

---

## 11. Edge Case Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| EC-001 | Pool with min=0, max=1, no initial connections | Edge | Lazy creation on first acquire |
| EC-002 | Pool with min=max=1 (single connection pool) | Edge | Serialization of access |
| EC-003 | Acquire and immediately release (no actual use) | Edge | Connection returned, use_count=1 |
| EC-004 | Release connection that was already evicted | Edge | Err or AlreadyClosed |
| EC-005 | Evict all connections, then acquire | Edge | New connection created |
| EC-006 | Shutdown on empty pool (no connections) | Edge | No error, idempotent |
| EC-007 | connection_timeout_ms = u64::MAX | Boundary | Acquire waits indefinitely (bounded by pool capacity) |
| EC-008 | max_pending_acquires = 0 (no queueing) | Edge | PoolExhausted immediately at capacity |
| EC-009 | Health check interval much shorter than idle timeout | Edge | Connections checked multiple times before timeout |
| EC-010 | Health check interval much longer than idle timeout | Edge | Connections may timeout between checks |
| EC-011 | Acquire during HalfOpen with circuit breaker | Edge | Test acquisition succeeds/fails correctly |
| EC-012 | Multiple pools with same NATS URLs | Isolation | Pools are independent |
| EC-013 | Pool with max_connections = 1 and 10 concurrent acquires | Contention | 1 succeeds, rest queue or exhaust |

---

## 12. Serde & Interop Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| SE-001 | PoolConfig JSON round-trip | Serde | Eq after deserialize |
| SE-002 | PoolStats JSON round-trip | Serde | Eq after deserialize |
| SE-003 | ConnectionPoolError JSON round-trip | Serde | Eq after deserialize |
| SE-004 | AcquireResult JSON round-trip | Serde | Eq after deserialize |
| SE-005 | HealthCheckResult JSON round-trip | Serde | Eq after deserialize |
| SE-006 | ConnectionStatus JSON round-trip | Serde | Eq after deserialize |
| SE-007 | CircuitBreakerState JSON round-trip | Serde | Eq after deserialize |
| SE-008 | EvictionReason JSON round-trip | Serde | Eq after deserialize |

---

## 13. Observability & Metrics Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| OB-001 | Circuit breaker state is accessible | Observability | Current state queryable |
| OB-002 | Pool stats include all required fields | Completeness | All PoolStats fields populated |
| OB-003 | Error context includes pool_id, timestamp, operation | Observability | ErrorContext populated |
| OB-004 | Error context optionally includes connection_id | Observability | connection_id: Some when applicable |
| OB-005 | Stats are safe to query concurrently | Concurrency | No data races |

---

## Test File Organization

```
crates/vo-worker/src/
  pool/
    mod.rs                          # Module root
    config.rs                       # PoolConfig + tests (PC-*)
    types.rs                        # ConnectionId, PoolId, ConnectionStatus, WaitHandle + tests (CI-*, PI-*, CS-*, WH-*)
    results.rs                      # AcquireResult, ReleaseResult, EvictResult, HealthCheckResult + tests (AR-*, HR-*)
    stats.rs                        # PoolStats + tests (PS-*)
    error.rs                        # ConnectionPoolError + tests (ET-*)
    circuit_breaker.rs              # CircuitBreakerState + logic + tests (CB-*)
    health_check.rs                 # Health check logic + tests (HC-*, IT-*)
    pool.rs                         # ConnectionPool trait/impl + lifecycle tests (PL-*, AQ-*, RL-*, EV-*, SD-*)
    pool_invariants.rs              # Invariant verification tests (IV-*)
    pool_lifecycle.rs               # Full lifecycle sequence tests (LC-*)
    pool_concurrent.rs              # Concurrent lifecycle tests (CL-*)
    pool_proptest.rs                # Property-based tests (PP-*)
    pool_sequences.rs               # Multi-operation sequence tests (SO-*)
    pool_edge_cases.rs              # Edge case tests (EC-*)
    pool_serde.rs                   # Serde tests (SE-*)
    pool_observability.rs           # Observability tests (OB-*)
```

## Test Count Summary

| Category | Count |
|----------|-------|
| Type construction & validation | 37 |
| Pool lifecycle (init/acquire/release/evict/shutdown) | 52 |
| Circuit breaker | 15 |
| Health check & idle timeout | 16 |
| Pool statistics | 8 |
| Invariant verification | 10 |
| Error taxonomy | 13 |
| Connection lifecycle | 13 |
| Property-based tests | 10 |
| Multi-operation sequences | 8 |
| Edge cases | 13 |
| Serde & interop | 8 |
| Observability & metrics | 5 |
| **Total** | **208** |
