# Findings: tw-0ktg - TimerId collision detection for scheduled timers

## Issue Summary
TimerId was generated from `workflow_id + step_index + timestamp` which caused two timers scheduled in the same millisecond for the same step to get identical IDs, leading to silent overwrite in the timer partition.

## Root Cause
The original design did not account for multiple timers being scheduled at the same timestamp for the same workflow step. UUID v5 is deterministic, so identical inputs would produce identical UUIDs.

## Fix Implemented
Located in `crates/vo-types/src/timer_id.rs`:

1. **Atomic counter per InstanceId**: Added `TimerIdGenerator` with a `Mutex<HashMap<InstanceId, Arc<CounterEntry>>>` where `CounterEntry` contains an `AtomicU64` counter.

2. **Counter appended to ID**: The `generate()` function now:
   - Gets/creates a counter entry for the instance_id
   - Atomically increments the counter via `fetch_add(1, Ordering::SeqCst)`
   - Creates UUID v5 from `instance_id-step_index-timestamp_ms`
   - Appends counter to UUID: `format!("{}-{}", uuid.to_string(), counter)`

3. **Concurrent test**: Added `timer_id_generator_concurrent_10000_timers_8_threads` test that:
   - Spawns 8 threads
   - Each thread creates 1250 timers (10000 total)
   - All threads use same instance_id, step_index, and timestamp_ms
   - Verifies zero duplicates

## Files Changed
- `crates/vo-types/src/timer_id.rs` - New file with TimerIdGenerator implementation
- `crates/vo-types/src/lib.rs` - Added `pub mod timer_id;` and `pub use timer_id::TimerIdGenerator;`

## Test Results
All 22 timer_id tests pass:
- Basic uniqueness tests (same instance, different steps, different timestamps, different instances)
- Concurrent collision tests with 8 threads generating 10000 timers total
- Same inputs concurrent test (10000 timers with identical instance/step/timestamp)

## Notes
- The signal_scope serialization test (`signal_scope_serializes_buffer_policy_when_present`) fails, but this is a pre-existing issue unrelated to this fix
- Clippy reports 0 errors, 1 warning (unrelated to timer_id)
