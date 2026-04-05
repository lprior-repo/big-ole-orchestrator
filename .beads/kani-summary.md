# Kani Model Checking Results

## vel-7ffu (dedupe_partition)

### verify_dedupe_key_rejects_empty
**Result**: compilation error (harness not reached)

**Error**: The harness `verify_dedupe_entry_rejects_empty_key_returns_invalid_argument` failed to compile because `verify_dedupe_key_roundtrip` in the same file has a compilation error that prevented the entire module from compiling.

### verify_dedupe_key_roundtrip
**Result**: compilation error

**Error** at `crates/vo-storage/src/dedupe_partition/verification.rs:14`:
```
error[E0277]: the trait bound `&str: kani::Arbitrary` is not satisfied
  --> crates/vo-storage/src/dedupe_partition/verification.rs:14:19
   |
14 |     let s: &str = kani::any();
   |                   ^^^^^^^^^^^ the trait `kani::Arbitrary` is not implemented for `&str`
```

**Root Cause**: The harness uses `kani::any()` to generate a symbolic `&str`, but Kani's `Arbitrary` trait is not implemented for `&str`. Solutions include using a `String` instead or implementing a custom `Arbitrary` for `&str`.

---

## vel-hqbq (effect_journal)

### verify_effect_id_rejects_empty
**Result**: compilation error (dependency failed)

**Error**: `vo-storage` failed to compile due to the `&str: kani::Arbitrary` error in `dedupe_partition/verification.rs`. This prevented the entire crate from compiling.

### verify_effect_key_encoding
**Result**: compilation error (dependency failed)

**Error**: Same as above - compilation of `vo-storage` failed due to the error in `dedupe_partition/verification.rs`.

---

## Summary

| Bead | Harness | Result | Notes |
|------|---------|--------|-------|
| vel-7ffu | verify_dedupe_key_rejects_empty | compilation error | Harness unreachable due to sibling module error |
| vel-7ffu | verify_dedupe_key_roundtrip | compilation error | `&str: kani::Arbitrary` not satisfied |
| vel-hqbq | verify_effect_id_rejects_empty | compilation error | Downstream victim of dedupe_partition error |
| vel-hqbq | verify_effect_key_encoding | compilation error | Downstream victim of dedupe_partition error |

**Fix Required**: Fix `dedupe_partition/verification.rs:14` to use `String` instead of `&str` for the symbolic variable, or implement a custom `kani::Arbitrary` for `&str`.
