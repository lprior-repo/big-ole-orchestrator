# Findings: tw-s8x4 - Concurrent Lease Acquire Integration Test

## Task
Add a concurrent integration test for `LeaseStore::acquire()` that spawns two threads racing on the same `(instance_id, step_id)` pair and asserts exactly one succeeds.

## Analysis

### File Location
`/home/lewis/gt/crates/vo-storage/src/lease_partition/tests_integration_acquire.rs`

### Implementation
Added test `concurrent_acquire_on_same_pair_exactly_one_wins` (AQ-19) that:
1. Creates an `InMemoryLeaseStore` wrapped in `Arc` for thread-safe sharing
2. Spawns two threads that concurrently call `acquire()` with the same `(instance_id, step_id)`
3. Asserts exactly one succeeds with fence token 1
4. Asserts the other fails with `LeaseStoreError::LeaseAlreadyHeld`

### Key Design Decisions
- **Thread-safe store**: `InMemoryLeaseStore` uses `std::sync::Mutex` and is `Sync`, unlike `DeterministicLeaseStore` which uses `RefCell`
- **Arc wrapping**: Allows both threads to access the same store instance
- **Cloning IDs**: `InstanceId` and `StepId` are not `Copy`, so we clone before moving into thread closures

### Test Verification
```
cargo test -p vo-storage concurrent_acquire_on_same_pair_exactly_one_wins
# Result: 1 passed, 1976 filtered out
```

## Notes
- The existing test `concurrent_acquire_on_same_pair_first_writer_wins` tests sequential (not concurrent) behavior
- The `InMemoryLeaseStore` properly serializes concurrent acquires via its mutex guards
