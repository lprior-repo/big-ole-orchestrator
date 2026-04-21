# ARCH-DRIFT Findings: vo-storage dedupe_partition

## Bead: ve-9i5v3
**Type**: task (Architecture Drift Check)
**Priority**: P2
**Assignee**: veloxide/polecats/mutant

## Architecture Analysis

### Documented Architecture (mod.rs:1-8)
```
Data (`AdmissionResult`, `DedupeEntry`, `DedupeStoreError`)
→ Calc (`encode_dedupe_key`, `decode_dedupe_key`, `encode_dedupe_entry`, `decode_dedupe_entry`)
→ Actions (`DedupeStore` trait)
```

### Layering Verification

| Layer | Components | Verified |
|-------|-----------|----------|
| Data | `AdmissionResult`, `DedupeEntry`, `DedupeStoreError`, `DedupeRetentionRecord` | ✅ All in mod.rs |
| Calc | `encode_dedupe_key`, `decode_dedupe_key`, `encode_dedupe_entry`, `decode_dedupe_entry`, `encode_dedupe_retention_record`, `decode_dedupe_retention_record` | ✅ All in mod.rs |
| Actions | `DedupeStore` trait, `FjallDedupeStore`, `InMemoryDedupeStore` | ✅ Trait in mod.rs, impls in separate files |

### File Structure
```
dedupe_partition/
├── mod.rs                      (392 lines) ⚠️ EXCEEDS 300 LINE LIMIT
├── fjall_dedupe.rs             (268 lines) ✅
├── in_memory_dedupe.rs         (101 lines) ✅
├── proptests.rs                (41 lines) ✅
├── red_queen_constants_expiry.rs (44 lines) ✅
├── red_queen_fjall_adversarial.rs (698 lines) ⚠️ EXCEEDS 300 LINE LIMIT
├── red_queen_serde_behavior.rs (145 lines) ✅
├── red_queen_validation.rs     (133 lines) ✅
├── verification.rs             (20 lines) ✅
└── tests/
    ├── mod.rs
    ├── tests_concurrent.rs
    ├── tests_encoding.rs
    ├── tests_entry_construction.rs
    ├── tests_exactly_once.rs
    ├── tests_mutation_killers.rs
    ├── tests_purge.rs
    └── tests_store_operations.rs
```

## Findings

### Violations

1. **File Size Violations** (<300 line limit):
   - `mod.rs`: 392 lines (exceeds by 92)
   - `red_queen_fjall_adversarial.rs`: 698 lines (exceeds by 398)

2. **Compilation Warning** (non-dedupe_partition issue):
   - `vo-storage/src/receipts/mod.rs:17`: Duplicate test module (`tests.rs` and `tests/mod.rs` conflict)
   - This prevents `cargo test` from completing but is unrelated to dedupe_partition

### Compliance

- ✅ Data-Calc-Actions layering is correctly implemented
- ✅ No cross-layer imports detected
- ✅ Error types use `#[non_exhaustive]` where appropriate
- ✅ `#[must_use]` annotations present on pure functions
- ✅ Test coverage with Red Queen adversarial tests
- ✅ Concurrency safety via striped Mutex in FjallDedupeStore
- ✅ Binary wire format for hot path (encode/decode_dedupe_entry)
- ✅ JSON encoding for retention records (encode/decode_dedupe_retention_record)

### Code Quality

- `cargo check`: Compiles with 2 warnings (unrelated to dedupe_partition)
- `cargo clippy`: Multiple warnings but no errors in dedupe_partition module
- `#[expect(clippy::expect_used)]` used appropriately on line 47 of fjall_dedupe.rs for system time expect

## Conclusion

**ARCH-DRIFT STATUS**: Minor drift detected

The dedupe_partition module correctly follows the Data-Calc-Actions architecture
documented in its module docstring. However, two files exceed the 300-line limit:

1. `mod.rs` (392 lines) - Core data/calc/actions layer
2. `red_queen_fjall_adversarial.rs` (698 lines) - Test/adversarial module

**Recommendation**: Consider splitting `mod.rs` into:
- `data.rs` (AdmissionResult, DedupeEntry, DedupeStoreError, DedupeRetentionRecord)
- `calc.rs` (encoding/decoding functions)
- `actions.rs` (DedupeStore trait definition)

And splitting `red_queen_fjall_adversarial.rs` into smaller adversarial test modules.

**No code changes required** - this is a QA/audit bead.