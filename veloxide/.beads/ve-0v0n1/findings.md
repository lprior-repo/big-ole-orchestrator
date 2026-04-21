# Findings: ve-0v0n1 - BLACKHAT: vo-sdk-macros — TaskDef args ordering preserved

## Issue
The bead identified a problem where generic functions and async functions with where clauses were incorrectly rejected by the `#[task]` macro.

## Root Cause
The `parse_task` function in `crates/vo-sdk-macros/src/task.rs` had a check that rejected all generic functions:

```rust
let has_generics = !parsed.sig.generics.params.is_empty()
    || parsed.sig.generics.lt_token.is_some()
    || parsed.sig.generics.where_clause.is_some();

if has_generics {
    return Err(Error::GenericFunctionNotSupported);
}
```

This caused two blackhat tests to fail:
- `bh_pass_async_where_clause` - async functions with where clauses
- `bh_pass_generic_fn` - generic functions

## Fix Applied
Removed the generic function rejection logic from `parse_task` in `task.rs`:

```rust
let _has_generics = !parsed.sig.generics.params.is_empty()
    || parsed.sig.generics.lt_token.is_some()
    || parsed.sig.generics.where_clause.is_some();
```

Also removed the `GenericFunctionNotSupported` error variant from `error.rs` and its corresponding match arm in `lib.rs`.

## Verification
All 71 tests in vo-sdk-macros now pass, including:
- 11 blackhat tests (previously 9 passed, 2 failed)
- 60 other tests (UI tests, unit tests, doc tests)

## Code Changes
1. `crates/vo-sdk-macros/src/task.rs:59-62` - Removed generic rejection check
2. `crates/vo-sdk-macros/src/error.rs:19-20` - Removed GenericFunctionNotSupported variant
3. `crates/vo-sdk-macros/src/lib.rs:92-93` - Removed GenericFunctionNotSupported match arm

## Notes
The existing test `generate_task_entrypoint_omits_generics_from_main_for_generic_task` in task.rs already documented the expected behavior: generic functions should be accepted, and the generated `main()` should be non-generic (which the current code already handles correctly via the `is_generic` check in `generate_task_entrypoint`).
