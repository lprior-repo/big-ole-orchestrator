# Findings: tw-jhzu - vo-storage lease acquire TOCTOU race fix

## Issue
CRITICAL ADR-029/043: `acquire()` in `FjallLeaseStore` had non-atomic operations causing TOCTOU race. Split-brain possible when two concurrent callers both see expired/no lease and both succeed in inserting.

## Root Cause
In `crates/vo-storage/src/lease_partition/fjall_lease_store.rs`:

The `acquire()` method (lines 140-181 original) performed these operations sequentially without atomicity:
1. `get_current_lease()` - reads lease (line 159)
2. Check if expired
3. `allocate_fence_token()` - allocates token (line 169)
4. `insert_lease()` - inserts new lease (line 178)

Between step 1 and step 4, another thread could:
- Also see expired/no lease
- Also allocate the same fence token
- Both insert leases = split-brain

The `InMemoryLeaseStore` already had proper locking (holds `leases` and `fences` mutexes for entire sequence), but `FjallLeaseStore` had no locking.

## Fix Applied
Added striped `parking_lot::Mutex` locking around the entire `acquire()` critical section, following the same pattern as `FjallDedupeStore`:

1. Added `NUM_STRIPES = 64` and `stripes: Vec<Mutex<()>>` to `FjallLeaseStore` struct
2. Added `stripe_for_key()` to hash (instance_id, step_id) to a stripe index
3. Modified `acquire()` to acquire the stripe lock before check, hold through allocate+insert

This ensures concurrent acquires for the SAME (instance_id, step_id) are serialized while allowing independent leases to proceed in parallel.

## Files Changed
- `crates/vo-storage/src/lease_partition/fjall_lease_store.rs`

## Changes
- Added `parking_lot::Mutex` import
- Added `NUM_STRIPES` and `stripes` field to struct
- Added `stripe_for_key()` helper
- Modified `open()` to initialize stripes
- Modified `acquire()` to lock stripe before check-fence-allocate-insert sequence

## Verification
- Library builds successfully: `cargo build --package vo-storage --lib`
- Note: Test compilation fails due to pre-existing issue (duplicate `tests` module in `crates/vo-storage/src/receipts/`) - not related to this fix

## Remaining Dead Code Warning
`delete_lease()` method is unused - the `release()` method directly calls `self.lease_partition.remove()`. This was pre-existing; could be cleaned up in a follow-up.
