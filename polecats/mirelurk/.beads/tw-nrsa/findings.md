# Findings: tw-nrsa - Eliminate WriteBudget and ReservedPermitBudget Duplication

## Executive Summary

**Bead**: tw-nrsa
**Title**: velocide: Eliminate WriteBudget and ReservedPermitBudget duplication
**Status**: QA/AUDIT COMPLETE - NO CODE CHANGES MADE

---

## 1. WriteBudget Duplication Analysis

### 1.1 Definitions Found

**Definition 1**: `vo-core/src/write_class.rs:140`
- **Access**: Public, exported via `pub mod write_class`
- **Uses**: `RefCell<u64>` for `critical_used`, `projection_used`, `blob_used`
- **API**: `new()`, `remaining()`, `can_write()`, `reserve()` → returns `Result<(), Error>`
- **Error Type**: `vo_core::write_class::Error::BudgetExceeded`
- **Tests**: Extensive tests in `write_class.rs:239-674`

**Definition 2**: `vo-storage/src/append.rs:85`
- **Access**: Public, exported via `pub mod append`
- **Uses**: `RefCell<u64>` for `critical_used`, `projection_used`, `blob_used`
- **API**: `new()`, `remaining()`, `can_write()`, `reserve()` → returns `Result<(), BudgetError>`
- **Error Type**: `BudgetError` defined at `append.rs:176`
- **Has**: `release()` method NOT in vo-core definition
- **Tests**: Extensive tests throughout append.rs

**Definition 3**: `vo-core/src/write_budget.rs:6`
- **Access**: PRIVATE/ORPHANED - NOT exported in lib.rs
- **Uses**: `RefCell<u64>`
- **Status**: DEAD CODE - never imported anywhere
- **Finding**: Orphaned file, can be safely deleted

### 1.2 RefCell Anti-Pattern Confirmed

Both active definitions use `RefCell<u64>`:
- `vo-core/src/write_class.rs:144-146`
- `vo-storage/src/append.rs:89-91`

**Problem**: `RefCell` is NOT `Sync`. Using `RefCell` in multi-threaded/actor context is UNSAFE.

### 1.3 API Differences Between Definitions

| Feature | vo-core | vo-storage |
|---------|---------|------------|
| `new()` | ✅ | ✅ |
| `remaining()` | ✅ | ✅ |
| `can_write()` | ✅ | ✅ |
| `reserve()` | ✅ | ✅ |
| `release()` | ❌ | ✅ |
| Error Type | `Error::BudgetExceeded` | `BudgetError` |

The vo-storage version has an additional `release()` method.

---

## 2. ReservedPermitBudget Analysis

### 2.1 Definition Found

**Definition**: `vo-actor/src/workload.rs:7`
- Re-exported via `vo-actor/src/lib.rs:13`: `pub use workload::{ReservedPermitBudget, WorkloadClass};`

**NOT FOUND in vo-core** despite bead description claiming "vo-core and vo-actor" duplication.

### 2.2 API

- Uses `std::collections::HashMap<WorkloadClass, u32>` for tracking
- Methods: `new()`, `try_acquire()`, `release()`, `reset()`, `available()`
- Uses `WorkloadClass` (not `WriteClass`)

### 2.3 Only ONE definition exists

**Finding**: The bead description appears outdated. `ReservedPermitBudget` is only defined in vo-actor, not in vo-core.

---

## 3. Thread Safety Analysis

### 3.1 WriteBudget

Using `RefCell<u64>` means `WriteBudget` is **NOT thread-safe**:
- `Clone` derive on line 139/85 suggests it may be cloned across actor instances
- If shared across threads, this is UNSAFE

### 3.2 ReservedPermitBudget

Uses `std::collections::HashMap` which is also NOT thread-safe for concurrent modification.

---

## 4. Recommendations

### 4.1 High Priority (RefCell Issue)

1. **Replace `RefCell<u64>` with `Cell<u64>`** in WriteBudget
   - `Cell<u64>` is thread-safe for single-threaded ownership
   - If multi-threaded sharing needed: use `AtomicU64`

2. **Consolidate WriteBudget definitions**
   - Move canonical definition to `vo-common`
   - Have vo-core and vo-storage re-export from vo-common

3. **Delete orphaned `vo-core/src/write_budget.rs`** (dead code)

### 4.2 Medium Priority (API Divergence)

4. **Add `release()` method to vo-core WriteBudget** to match vo-storage API

5. **Update bead description** to remove incorrect "vo-core and vo-actor" claim for ReservedPermitBudget

---

## 5. Files Requiring Changes

If implementing the fix:

| File | Action |
|------|--------|
| `vo-common/src/budget.rs` (new) | Create with canonical WriteBudget using `Cell<u64>` |
| `vo-core/src/write_class.rs` | Remove WriteBudget definition, re-export from vo-common |
| `vo-core/src/write_budget.rs` | DELETE (orphaned dead code) |
| `vo-storage/src/append.rs` | Remove WriteBudget definition, re-export from vo-common |
| `vo-actor/src/workload.rs` | No changes needed (only definition) |
| `vo-actor/src/workload.rs` | Update to use Cell/Atomic for thread safety |

---

## 6. Verification Commands

```bash
# Verify no Sync issues after fix
cd /home/lewis/src/veloxide
cargo check --all-features 2>&1 | grep -i "refcell\|sync"

# Verify no orphan write_budget.rs
grep -r "mod write_budget" crates/vo-core/src/

# Check for Send+Sync on WriteBudget after fix
cargo check --message-format=json 2>&1 | grep -i "send\|sync"
```

---

## Conclusion

**This is an AUDIT bead**. The duplication and RefCell anti-pattern are real issues that should be fixed, but this bead is filed as a TODO/audit task rather than an immediate implementation task.

**Recommended Action**: File a new implementation bead with the specific fix requirements identified above.
