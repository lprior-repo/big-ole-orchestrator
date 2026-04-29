# Red Queen Test Plan: vo-core

## Executive Summary

The Red Queen adversarial testing process was run against vo-core. The process identified **14 failing quality gates** representing systemic issues in code quality, test integrity, and security posture.

**Status**: CROWN FORFEIT — All 14 validation checks failed

## Findings (Priority Order)

### CRITICAL (Must Fix Immediately)

| ID | Dimension | Finding | Command |
|----|-----------|---------|---------|
| GEN-1-5 | fp-gate-tests | `cargo test` fails to compile | `cd crates/vo-core && cargo test` |

**Root Cause**: Tests are out of sync with implementation
- `ReplayError::PayloadDecodeFailed` uses `detail` field but tests reference `source`
- `ReplayError::kind()` method is called but never implemented

### MAJOR (High Priority)

| ID | Dimension | Finding | Count |
|----|-----------|---------|-------|
| GEN-1-1 | fp-gate-no-panic | 101 unwrap/expect/panic violations | 101 |
| GEN-1-2 | fp-gate-exhaustive | Wildcard enum match arms | 101 |
| GEN-1-4 | fp-gate-lint | Clippy warnings as errors | 101 |
| GEN-1-6 | quality-dry | DRY violations (redundant_clone, manual_map, unnecessary_wraps) | 101 |
| GEN-1-7 | fowler-dead-code | Dead code / unused imports | 101 |
| GEN-1-8 | fowler-dry | DRY violations | 101 |
| GEN-1-9 | fowler-error-handling | unwrap/expect error handling | 101 |
| GEN-1-10 | fowler-exhaustive | Wildcard enum matches | 101 |
| GEN-1-11 | fowler-test-coverage | Coverage below 80% | - |
| GEN-1-12 | fowler-security | Security vulnerabilities (cargo audit) | 2 |
| GEN-1-13 | fowler-licenses | License check failures | 4 |

### MINOR

| ID | Dimension | Finding |
|----|-----------|---------|
| GEN-1-3 | fp-gate-format | Formatting issues |

## Fix Strategy

### Phase 1: Fix Test Compilation (Immediate)

**File**: `crates/vo-core/src/invalid_business_data_tests.rs`
- Line 765: Change `source: "invalid UTF-8".to_string()` to `detail: "invalid UTF-8".to_string()`

**File**: `crates/vo-core/src/replay/error_tests.rs`
- Lines 157, 168, 179, 187, 198: Remove `err.kind()` calls OR implement `kind()` method on `ReplayError`

**Implementation Option A**: Implement `kind()` method
```rust
impl ReplayError {
    pub fn kind(&self) -> ReplayErrorKind {
        match self {
            ReplayError::InstanceMismatch { .. } => ReplayErrorKind::Deterministic,
            ReplayError::SequenceGap { .. } => ReplayErrorKind::Deterministic,
            ReplayError::SequenceDuplicate { .. } => ReplayErrorKind::Deterministic,
            ReplayError::PayloadDecodeFailed { .. } => ReplayErrorKind::Deterministic,
            ReplayError::TransitionFailed { .. } => ReplayErrorKind::Deterministic,
            ReplayError::UnexpectedEventType { .. } => ReplayErrorKind::Deterministic,
            ReplayError::UpcastingFailed { .. } => ReplayErrorKind::Deterministic,
            ReplayError::BlobPublicationFailed { .. } => ReplayErrorKind::Deterministic,
        }
    }
}
```

**Implementation Option B**: Remove `kind()` calls from tests (if method not intended)

### Phase 2: Format and Lint Fixes

```bash
cd crates/vo-core && cargo fmt
cd crates/vo-core && cargo clippy --fix --allow-dirty
```

### Phase 3: Security and License Audit

Review `cargo audit` and `cargo deny` output for actual vulnerabilities.

## Verification

After fixes, run:
```bash
cd crates/vo-core && cargo test
cargo llvm-cov --fail-under-lines 80
```

## Deterministic State

- **Generation**: 1
- **Lineage Size**: 14 checks
- **Survivors**: 13 (quality gates) + 1 (tests)
- **Zero-Streak**: 1 (generation 1 ended with 0 survivors — all were pre-existing failures)
