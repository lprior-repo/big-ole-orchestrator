# Findings: tw-7egm - Connection Pool Health Check Implementation

## Issue
The connection pool in vo-common/src/connection_pool/mod.rs and vo-worker/src/pool/pool.rs held connections but never checked if they were still alive. After a server restart, all pooled connections would be dead but the pool would still return them, causing errors on use.

## Fix Applied

### 1. Added `max_retries` to PoolConfig

**vo-common/src/connection_pool/mod.rs:24-32**
- Added `max_retries: u32` field to `PoolConfig` struct

**vo-worker/src/pool/config.rs:6-13**
- Added `max_retries: u32` field to `PoolConfig` struct
- Updated `PoolConfig::new()` to accept 7th parameter
- Updated `with_defaults()` to set `max_retries: 3`
- Updated `From<PoolConfig> for VoPoolConfig` and vice versa

### 2. Added `health_check()` method to NatsConnectionWrapper

**vo-worker/src/pool/pool.rs:43-52**
- Added `health_check(&self) -> HealthCheckResult` method
- Returns `HealthCheckResult::Healthy` if `is_healthy()`, else `HealthCheckResult::Stale`

### 3. Modified `acquire_with_timeout()` to run health checks

**vo-worker/src/pool/pool.rs:172-274**
- Added retry loop with `max_retries` limit
- When acquiring an idle connection, runs `health_check.check_connection()` on it
- If health check fails (not Healthy), evicts the connection and retries with next idle connection
- Tracks `total_health_checks` and `failed_health_checks` in stats
- Uses `EvictionReason::HealthCheckFailed(HealthCheckResult::Stale)` for evicted connections

### 4. Updated All PoolConfig Usages

Updated all `PoolConfig::new()` calls throughout the codebase to include the new `max_retries` parameter:
- vo-common/src/connection_pool/mod.rs (tests)
- vo-worker/src/pool/config.rs (tests)
- vo-worker/src/pool/pool.rs (tests)
- vo-worker/src/pool/managed_pool.rs (tests)
- vo-worker/tests/integration_lifecycle_tests.rs
- vo-worker/tests/proptest_suite.rs

## Verification

```bash
cargo test -p vo-worker --lib     # 209 passed
cargo test -p vo-worker --test integration_lifecycle_tests  # 23 passed
cargo test -p vo-common           # 314 passed
```

## Key Changes

| File | Change |
|------|--------|
| vo-common/src/connection_pool/mod.rs | Added `max_retries` field to PoolConfig |
| vo-worker/src/pool/config.rs | Added `max_retries` to PoolConfig, updated constructors |
| vo-worker/src/pool/pool.rs | Added `health_check()` to NatsConnectionWrapper, modified `acquire_with_timeout()` to run health checks with retry loop |

## How It Works

1. On `acquire()`, the pool now loops through idle connections up to `max_retries` times
2. For each idle connection, it runs `health_check.check_connection()` which verifies:
   - Connection is not stale based on `last_used_at` vs `idle_timeout_ms`
3. If health check fails, the connection is evicted and the pool tries the next idle connection
4. If all idle connections fail health checks, a new connection is created (if under `max_connections`)
5. If no new connections can be created and all retries exhausted, returns `PoolExhausted`
