# Bevel-9kxy: Adaptive Pool Scaling Implementation

## Summary

Implemented bounded worker pool with adaptive scaling in `vo-worker/src/pool/pool.rs`.
The connection pool now dynamically adjusts its target connection count based on demand
signals, growing during high load and shrinking during idle periods.

## Changes Made

### New Types (pool.rs)

1. **`DemandSignal`** enum - Indicates pool pressure level:
   - `High` - Pool is under pressure, should grow
   - `Normal` - Pool is balanced, no action needed
   - `Low` - Pool has excess capacity, should shrink

2. **`ScaleResult`** enum - Result of a scaling decision:
   - `ScaledUp { new_target }` - Pool scaled up
   - `ScaledDown { new_target }` - Pool scaled down
   - `CooldownRemaining` - Cooldown period still active
   - `NoAction` - No scaling needed

3. **`PoolScaler`** struct - Manages adaptive scaling:
   - `last_scale_at: Instant` - Timestamp of last scaling decision
   - `scale_cooldown: Duration` - Min time between scaling decisions (default 5s)
   - `current_target: u32` - Target connection count (starts at min_connections)

### Integration

- Added `PoolScaler` field to `PoolState`
- Integrated `try_scale(DemandSignal::High)` into `acquire_with_timeout()` - called on every acquire
- Added `create_connection()` method - creates new idle connections for the scaler to use
- Added `start_scaling_task()` method - spawns background tokio task for periodic scale-down checks (30s interval)
- Removed `Clone` derive from `PoolState` (PoolScaler contains non-Clone `Instant`)

### Scaling Logic (`try_scale`)

**Scale Up (High demand):**
- Cooldown check: if elapsed since last scale < cooldown, return `CooldownRemaining`
- If `current_target >= max_connections`, return `NoAction`
- If `idle_count + pending_count > current_target * 0.8`, increment target by 1
- Call `create_connection()` to grow pool immediately
- Update `last_scale_at` to current time

**Scale Down (Low demand):**
- If `current_target <= min_connections`, return `NoAction`
- If `idle_count > current_target * 0.5`, decrement target by 1
- Update `last_scale_at` to current time

**Normal demand:** Always returns `NoAction`

### Exports (mod.rs)

Exported new types: `DemandSignal`, `PoolScaler`, `ScaleResult`

### Tests (6 new)

1. `test_scale_up_on_high_demand` - 4 idle connections, High signal → ScaledUp to target 3
2. `test_scale_down_on_idle` - 5 idle connections, target=5, Low signal → ScaledDown to target 4
3. `test_scale_cooldown_enforced` - First scale succeeds, second within cooldown → CooldownRemaining
4. `test_scale_respects_max_connections` - Target at max → NoAction regardless of demand
5. `test_scale_respects_min_connections` - Target at min → NoAction on Low signal
6. `test_scale_normal_signal_no_action` - Normal signal always returns NoAction

### Pre-existing Bug Fix

Fixed workspace dependency `bloom = "0.6"` → `bloom = "0.3"` (0.6 doesn't exist on crates.io)

## Test Results

- All 432 vo-worker tests pass (432 passed, 1 ignored)
- 6 new scaler tests added and passing
- No regressions in existing pool or pool tests

## Files Modified

- `crates/vo-worker/src/pool/pool.rs` - Core implementation (+3 new types, 3 new methods, 6 tests)
- `crates/vo-worker/src/pool/mod.rs` - Exports
- `Cargo.toml` - Fixed bloom dependency version
