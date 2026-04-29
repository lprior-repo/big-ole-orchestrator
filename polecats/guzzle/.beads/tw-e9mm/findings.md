# Findings: tw-e9mm - SlotBudget Implementation

## Issue
vo-core: Admission controller must reject workflows that exceed slot budget

## Problem
The admission controller tracked running workflows by count but not by resource budget. A single workflow with 1000 parallel steps could starve all others.

## Solution
Added `SlotBudget` struct to workload.rs that tracks total slots and used slots at the workflow level.

### Implementation

**New Types** (in `crates/vo-core/src/admission/workload.rs`):
- `SlotBudget` - struct with `total_slots` and `used_slots` fields
- `SlotBudgetCheckResult` - enum with `Accepted` and `Rejected` variants
- `SlotBudgetRejectionReason` - enum with `InsufficientSlots` variant

**New Functions**:
- `SlotBudget::new(total_slots)` - Create new budget with specified total slots
- `SlotBudget::remaining()` - Get remaining slots
- `SlotBudget::can_acquire(slots)` - Check if slots can be acquired
- `check_slot_budget(budget, required_slots)` - Pure function to check budget
- `reserve_slot_budget(budget, required_slots)` - Reserve slots for workflow
- `release_slot_budget(budget, slots)` - Release slots when workflow completes
- `calculate_workflow_slots(step_count, parallelism_factor)` - Calculate slots needed

**Exports** (in `mod.rs`):
Added exports for all new SlotBudget types and functions.

**Tests** (11 new tests in `workload_tests.rs`):
- `slot_budget_default_is_100_slots`
- `slot_budget_new_with_custom_total`
- `slot_budget_can_acquire_true_when_slots_available`
- `slot_budget_can_acquire_false_when_insufficient`
- `slot_budget_reserve_success`
- `slot_budget_reserve_rejected_when_insufficient`
- `slot_budget_release_frees_slots`
- `slot_budget_release_never_negative`
- `slot_budget_check_accepts_sufficient_slots`
- `slot_budget_check_rejects_insufficient_slots`
- `slot_budget_workflow_admission_test` - Test: 100 slots, workflow needing 80, second needing 30 rejected
- `calculate_workflow_slots_multiplies_step_count_by_parallelism`
- `calculate_workflow_slots_minimum_one_slot`

## Verification
- All 11 new slot budget tests pass
- `cargo clippy -p vo-core` passes with 0 errors
- `cargo test -p vo-core workload_tests::slot_budget` - 11 passed

## Files Changed
- `crates/vo-core/src/admission/workload.rs` - Added SlotBudget implementation
- `crates/vo-core/src/admission/workload_tests.rs` - Added tests
- `crates/vo-core/src/admission/mod.rs` - Added exports