# Implementation Summary

## Bead: wtf-rbay
## Title: implement wtf-linter WTF-L006: std-thread-spawn-in-workflow

## What was implemented

- Added unit tests directly in `crates/wtf-linter/src/l006.rs` to close contract/test parity gaps:
  - emits no diagnostic when no `std::thread::spawn` exists
  - emits L006 diagnostic when `std::thread::spawn` appears in workflow `execute`
  - emits no diagnostic for non-workflow async helper functions
  - emits one diagnostic per violation for multiple spawn calls
  - returns parse error on invalid Rust source

These tests validate the existing L006 visitor behavior against contract postconditions Q1-Q5 and invariant I2.

## Contract adherence notes

- Workflow-only detection remains scoped to async `execute` in impl blocks.
- Nested and repeated expressions are already traversed by visitor recursion; tests now assert this behavior for multi-violation cases.
- Diagnostics verified to carry `LintCode::L006` and suggestion text.

## Verification

- `cargo test -p wtf-linter l006 -- --nocapture`

## Files changed

- `crates/wtf-linter/src/l006.rs`
