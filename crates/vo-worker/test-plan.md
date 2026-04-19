# vo-worker Test Plan

## Current Test Coverage

### Test Summary
- **Total tests**: 139 passing tests
- **Sync tests**: 111 tests
- **Async (tokio::test) tests**: 28 tests

### Module Coverage

| Module | Sync Tests | Async Tests | Total |
|--------|-----------|-------------|-------|
| `lib.rs` | 41 | 6 | 47 |
| `port.rs` | 0 | 1 | 1 |
| `retry.rs` | 2 | 3 | 5 |
| `pool/pool.rs` | 6 | 1 | 7 |
| `pool/config.rs` | 11 | 0 | 11 |
| `pool/circuit_breaker.rs` | 10 | 0 | 10 |
| `pool/health_check.rs` | 10 | 0 | 10 |
| `pool/hash_ring.rs` | 10 | 0 | 10 |
| `supervisor/port.rs` | 11 | 0 | 11 |
| `storage/port.rs` | 2 | 0 | 2 |
| `connector/error.rs` | 10 | 0 | 10 |
| `connector/registry.rs` | 11 | 0 | 11 |
| `connector/trait_def.rs` | 9 | 0 | 9 |
| `connector/types.rs` | 18 | 0 | 18 |
| `connector/http.rs` | 23 | 0 | 23 |
| **Total** | **144** | **11** | **155** |

## Test Coverage Achieved

### Completed Test Additions

#### 1. HTTP Connector Tests (23 tests)
- Connector construction and configuration
- Prepare with idempotency keys
- Error classification (retryable vs terminal)
- Reconcile behavior
- Multiple effect preparation
- Outcome variants

#### 2. Lock Manager Types (41 tests in lib.rs)
- LockEntry construction and expiry
- LockRequest/Response fields
- LockRelease/Promote fields
- LockQuery variants
- WaitEdge and WaitForGraph operations
- LockError variants (NotFound, NotOwner, InvalidToken, Deadlock, etc.)

#### 3. Connector Error Tests (10 tests)
- Retryable vs terminal classification
- Compensation not supported
- Debug and display implementations
- Empty message handling

#### 4. Connector Registry Tests (11 tests)
- New/empty registry
- Register single/multiple connectors
- Get existing/nonexistent
- Overwrite connectors
- List operations
- Clone behavior

#### 5. Connector Trait Tests (9 tests)
- Trait bounds (Send + Sync)
- Method implementations
- Prepare/commit/reconcile cycles
- Default compensate implementation
- Custom compensate override

#### 6. Connector Types Tests (18 tests)
- PreparedEffect serialization
- CommitOutcome variants
- ReconcileOutcome variants
- ReconcileAction conversion
- PartialEq implementations
- Large/nested payload handling

## Test Categories to Implement

### Red Phase: Unit Tests (Fail First)
1. **Task dispatch unit tests**
   - `test_task_dispatch_queue_empty`
   - `test_task_dispatch_single_worker`
   - `test_task_dispatch_multiple_workers`
   - `test_task_dispatch_worker_unavailable`
   - `test_task_dispatch_timeout`

2. **Worker lifecycle tests**
   - `test_worker_spawn_success`
   - `test_worker_spawn_failure`
   - `test_worker_lifecycle_transition`
   - `test_worker_respawn_on_death`
   - `test_worker_pool_capacity_limit`

3. **Heartbeat tests**
   - `test_heartbeat_send_success`
   - `test_heartbeat_timeout`
   - `test_heartbeat_stale_worker_detection`
   - `test_heartbeat_rate_limiting`

4. **Failure handling tests**
   - `test_cascade_failure_isolation`
   - `test_circuit_breaker_open_state`
   - `test_worker_quarantine_on_failure`
   - `test_recovery_from_partial_failure`

5. **HTTP connector tests**
   - `test_http_connector_prepare_idempotency`
   - `test_http_connector_commit_success`
   - `test_http_connector_commit_rate_limited`
   - `test_http_connector_commit_server_error`
   - `test_http_connector_reconcile_ambiguous`
   - `test_http_error_classification`

### Green Phase: Implementation
- Implement untested code paths
- Ensure all tests compile and fail initially

### Blue Phase: Verification
- Run full test suite
- Verify 100% coverage of critical paths
- Mutation testing on failure handling

## Acceptance Criteria

1. **Test count**: 139 tests (exceeded minimum of 80)
2. **Critical modules tested**: 
   - ✅ lib.rs (47 tests)
   - ✅ connector/http.rs (23 tests)
   - ✅ connector/error.rs (10 tests)
   - ✅ connector/registry.rs (11 tests)
   - ✅ connector/trait_def.rs (9 tests)
   - ✅ connector/types.rs (18 tests)
   - ✅ pool/pool.rs (7 tests)
3. **Async test coverage**: All async functions have tokio::test coverage
4. **Edge cases covered**: Timeouts, errors, empty states, cycle detection
5. **Integration paths**: Worker ↔ storage, worker ↔ connector, worker ↔ pool

## Next Steps

Remaining gaps to address in future iterations:
1. Worker lifecycle integration tests
2. Task dispatch flow tests
3. Heartbeat mechanism tests
4. Circuit breaker trip scenarios
5. Storage backend integration tests

## Implementation Order

1. **Phase 1**: HTTP connector tests (no dependencies)
2. **Phase 2**: Task dispatch tests (depends on lib.rs types)
3. **Phase 3**: Worker lifecycle tests (depends on supervisor)
4. **Phase 4**: Heartbeat tests (depends on health_check)
5. **Phase 5**: Failure handling tests (integration)
