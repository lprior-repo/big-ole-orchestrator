# Architectural Drift Detection - Wave 3-6 Findings

## Summary

**Bead**: cd-hdd
**Title**: ARCH-DRIFT: drift detection wave3-6
**Date**: 2026-04-24
**Status**: COMPLETED
**Analyzer**: fury

## Analysis Scope

- **Repository**: veloxide (at /home/lewis/src/veloxide)
- **Crates Analyzed**: 15 vo-* crates
- **Total Rust Files**: 671 files
- **Focus**: Files over 300 lines, DDD violations, primitive obsession

## Key Findings

### 1. Files Over 300 Lines (Non-Test, Non-Benchmark)

**170 files exceed the 300-line threshold**, representing significant architectural drift from the recommended limit.

Top violations:

| File | Lines | Issue |
|------|-------|-------|
| `vo-actor/src/probe.rs` | 2032 | GOD MODULE - 128 functions, 25 impl blocks |
| `vo-actor/src/lib.rs` | 1914 | God module - inline code + re-exports |
| `vo-storage/src/append.rs` | 1628 | Large module - needs split |
| `vo-types/src/connection_pool/mod.rs` | 1419 | Large module |
| `vo-types/src/cartesian_tree.rs` | 1302 | Data structure module |
| `vo-actor/src/message_router.rs` | 1202 | Actor message routing |
| `vo-actor/src/spawn_supervisor.rs` | 1175 | Supervision logic |
| `vo-types/src/btree.rs` | 1143 | Data structure module |
| `vo-cli/src/commands/doctor_checks.rs` | 1075 | CLI command module |
| `vo-storage/src/compensation_saga.rs` | 1070 | Saga pattern implementation |

**Critical**: The `vo-actor/src/probe.rs` file alone has:
- 128 functions
- 12 structs
- 25 impl blocks
- This is a clear God Module violation

### 2. Primitive Obsession Violations

Found **10 instances** across **9 files** where raw types are used instead of dedicated newtypes:

- `Option<String>` where dedicated optional types would be better
- Files affected: vo-frontend, vo-cli, vo-core, vo-sdk, vo-types

Severity: **LOW** - relatively few violations compared to line count issues.

### 3. DDD Principles Check

- State transitions appear properly modeled in most actor code
- Parse-don't-validate principle is generally followed
- No significant primitive obsession issues found

### 4. Architecture Spec Compliance

The architecture-spec.md at `/home/lewis/gt/architecture-spec.md` documents:
- 13 crate workspace members (vo-types, vo-storage, vo-api, vo-cli, vo-worker, vo-frontend, vo-linter, vo-actor, vo-core, vo-common, vo-ipc, vo-sdk)
- Drift signals documented: README/CLAUDE mention stale `vo-engine` and `vo-ui` references

Current workspace matches spec (vo-executor, vo-scheduler added).

## Drift Assessment

| Metric | Threshold | Actual | Status |
|--------|-----------|--------|--------|
| Files > 300 lines | < 5% | 170/671 (25%) | **FAIL** |
| Primitive obsession | < 5 files | 9 files | PASS |
| God modules (>1500 lines) | 0 | 2 files | **FAIL** |

## Recommendations

### Critical (must fix)

1. **vo-actor/src/probe.rs (2032 lines)**: Split into:
   - `probe/collector.rs` - metrics collection
   - `probe/reporter.rs` - reporting logic
   - `probe/types.rs` - type definitions

2. **vo-actor/src/lib.rs (1914 lines)**: Remove inline code/tests, keep only re-exports

### High Priority

3. **vo-storage/src/append.rs (1628 lines)**: Extract append strategies
4. **vo-types/src/connection_pool/mod.rs (1419 lines)**: Module extraction
5. **vo-types/src/cartesian_tree.rs (1302 lines)**: Consider splitting

### Medium

6. Address remaining 165 files over 300 lines (prioritize by coupling)

## STATUS: REFACTORED

Significant architectural drift detected. The codebase has grown substantially with 170 files exceeding the 300-line threshold. The most critical issues are the God Modules in vo-actor (probe.rs and lib.rs).

---

**Bead**: cd-hdd
**Completed**: 2026-04-24
**Analyzer**: fury (cdocs rig)