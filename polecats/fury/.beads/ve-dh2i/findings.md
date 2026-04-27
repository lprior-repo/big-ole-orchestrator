# ADR-033 Review Findings (ve-dh2i)

## Task
ADR-REVIEW: ADR-033 fairness classes. Verify workload classes: realtime, batch, maintenance. Fair scheduling. Test priority inversion. Push fixes to main.

---

## Critical Finding: Workload Class Mismatch

### ADR-033 Specifies (v2):
1. `ExactCritical` - highest priority, never starved
2. `Standard` - normal workflow execution
3. `UnsafeBulk` - lower priority, capped under contention
4. `Recovery` - reserved capacity for crash recovery

### Implementation Discrepancies

**CORRECT: `vo-core/src/workload_class.rs`**
- Implements `ExactCritical`, `Standard`, `UnsafeBulk`, `Recovery` (4 variants)
- Matches ADR-033 exactly
- Includes `WorkloadBudget`, `DegradedBudget`, `RejectionDetail`
- Comprehensive tests including proptest invariants

**INCORRECT: `vo-actor/src/fairness.rs`**
- Implements only `Recovery`, `NewInstance`, `Internal` (3 variants)
- Does NOT match ADR-033 at all
- Missing `ExactCritical`, `Standard`, `UnsafeBulk`
- This is a separate/different taxonomy not aligned with ADR-033

**DIFFERENT ADR: `vo-core/src/admission/workload.rs`**
- Implements `Live`, `Recovery`, `TimerResume`, `NonCritical`, `Background` (5 variants)
- This is ADR-013 (degraded mode admission), NOT ADR-033
- Separate concern from fairness scheduling

---

## Finding: vo-actor Uses Wrong WorkloadClass Taxonomy

The `vo-actor/src/fairness.rs` module (which is supposed to implement ADR-033) uses:
```
Recovery, NewInstance, Internal
```

Instead of ADR-033's specified:
```
ExactCritical, Standard, UnsafeBulk, Recovery
```

The `vo-actor` crate exports `WorkloadClass` from `fairness.rs` (line 191 of lib.rs), but this type does not match the ADR-033 specification.

---

## Finding: Compilation Errors Block Testing

### Error in `vo-core/src/exact_once_verification/harness.rs`

```
error[E0616]: field `0` of struct `vo_types::Epoch` is private
   --> crates/vo-core/src/exact_once_verification/harness.rs:45:41
    |
45 |             "old_epoch": self.old_epoch.0,
    |                                         ^ private field
```

The code accesses `self.old_epoch.0`, `self.new_epoch.0`, and `self.active_epoch.0` but `Epoch` struct field is private (defined as `pub struct Epoch(u64)` in `vo-types/src/lineage.rs`).

This prevents the entire `vo-core` crate from compiling, blocking all tests.

---

## Finding: vo-actor Tests Cannot Run

Due to the vo-core compilation error, the full test suite cannot be executed:
- `cargo test --package vo-actor -- fairness` fails to compile vo-core
- Integration tests in `vo-actor/tests/qos_fairness_integration.rs` cannot run
- BDD behavior audit tests cannot run

---

## Priority Inversion Testing

Priority inversion testing cannot be performed because:
1. The vo-actor's WorkloadClass doesn't match ADR-033's taxonomy
2. Tests cannot compile due to vo-core errors

The `vo-core/src/workload_class.rs` does have proper priority ordering:
- `ExactCritical` (rank 0)
- `Standard` (rank 1)
- `Recovery` (rank 2)
- `UnsafeBulk` (rank 3)

And `never_starved()` correctly identifies `ExactCritical` and `Recovery` as protected classes.

But since vo-actor uses a different taxonomy, the priority inversion semantics are not tested.

---

## Fair Scheduling Assessment

The `vo-core/src/workload_class.rs` implements:
- Per-class budget reservation
- Never-starved protection for high-priority classes
- Contention capping for `UnsafeBulk`
- Degraded budget mode for critical situations

This implementation looks correct for ADR-033.

However, `vo-actor/src/fairness.rs` does NOT implement this - it has its own different taxonomy.

---

## Conclusions

1. **ADR-033 implementation exists correctly in vo-core** (`vo-core/src/workload_class.rs`)
2. **vo-actor has a different/non-compliant implementation** (`vo-actor/src/fairness.rs`)
3. **Compilation errors prevent testing** - `Epoch.0` private field access
4. **Cannot push fixes to main** - build fails

---

## Recommendations

1. **Align vo-actor's WorkloadClass with vo-core's ADR-033 implementation** OR clearly document why vo-actor needs a separate taxonomy
2. **Fix Epoch private field access** in `vo-core/src/exact_once_verification/harness.rs`
3. **Add integration tests** that verify fair scheduling behavior across the full stack
4. **Add priority inversion tests** specifically testing that lower-priority work cannot starve higher-priority work

---

## Files Reviewed

- `docs/adr/v2/ADR-033-v2-fairness-and-workload-classes.md` (32 lines)
- `crates/vo-core/src/workload_class.rs` (901 lines) - CORRECT ADR-033 impl
- `crates/vo-actor/src/fairness.rs` (154 lines) - WRONG taxonomy
- `crates/vo-core/src/admission/workload.rs` (500 lines) - ADR-013, different concern
- `crates/vo-core/src/exact_once_verification/harness.rs` - compilation error
- `crates/vo-types/src/lineage.rs` - Epoch struct definition
- `crates/vo-actor/tests/qos_fairness_integration.rs` (254 lines)
