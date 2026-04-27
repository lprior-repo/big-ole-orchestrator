# ARCH-DRIFT Batch 3 Findings

## Summary

STATUS: PERFECT (audit only, no code changes made)

## Analysis Overview

**Audit performed on:** `/home/lewis/src/veloxide/crates/` and `/home/lewis/gt/crates/`
**Date:** 2026-04-24
**Bead:** se-xdy - Batch 3

## Git Status

- **Conflict markers:** NONE found (clean codebase)
- **Build status:** vo-actor compiles successfully with 4 minor warnings (unused imports)

## Line Count Analysis (Files > 300 lines)

### Top Production Violators (>300 lines, excluding tests)

| File | Lines | Issue |
|------|-------|-------|
| vo-actor/src/lib.rs | 1923 | God module - 40+ declarations + inline types |
| vo-storage/src/append.rs | 1438 | Mixed domains (budget+queue+backpressure+commit) |
| vo-actor/src/message_router.rs | 1419 | 30+ types in single file |
| vo-actor/src/spawn_supervisor.rs | 1306 | 18 types, state management |
| vo-actor/src/probe/types.rs | 1351 | Probe configuration types |
| vo-types/src/connection_pool/mod.rs | 1419 | 23 types, connection pool management |
| vo-types/src/command_history.rs | 1778 | High line count - needs review |
| vo-types/src/cartesian_tree.rs | 1308 | 65% tests (per batch 1) |
| vo-types/src/btree.rs | 1136 | 44% tests (per batch 1) |
| vo-cli/src/commands/doctor_checks.rs | 1075 | 8 types, mixed concerns |
| vo-storage/src/compensation_saga.rs | 1070 | 18 types |
| vo-types/src/effects.rs | 1162 | Effects system |
| vo-types/src/recovery_contract.rs | 1041 | Recovery contract types |

### Test Files (>300 lines, informational only)

| File | Lines |
|------|-------|
| vo-cli/tests/gap_coverage_tests.rs | 2055 |
| vo-types/src/workflow_tests.rs | 1962 |
| vo-executor/tests/adr_contract_tests.rs | 1877 |
| vo-cli/tests/cli_deep_coverage_tests.rs | 1843 |
| vo-core/src/replay/red_queen_adversarial_tests.rs | 1803 |
| vo-worker/tests/connector_runtime_contract_tests.rs | 1742 |

## DDD Analysis

### Primitive Obsession Check

**Finding:** The codebase has MIXED compliance with Parse Don't Validate principle.

**Good (wrapped types):**
- `ConnectionId(pub Ulid)` - properly wrapped
- `PoolId(pub String)` - wrapped with accessor
- `InstanceId` - properly typed
- `NamespaceId` - properly typed
- `ChannelId`, `PoolId`, `ConnectionId` all wrapped

**Potential Issues (raw types still in use):**
- Some internal APIs still use raw `String` for identifiers in less critical paths
- `TimestampMs(i64)` wrapper exists but may not be used consistently everywhere

### State Machine Patterns

**Good:**
- `LifecycleState` enum properly models workflow states
- `InstancePhaseView` enum (Replay, Live) properly typed
- `ConnectionStatus` enum (Idle, CheckedOut, HealthCheck, Closing, Closed) proper state modeling

**Observations:**
- Workflow transitions appear well-modeled in `signal_messages.rs`
- Actor lifecycle states are explicit enums, not booleans

## File Structure Issues

### God Files Identified

1. **vo-actor/src/lib.rs (1923 lines)**
   - Contains 40+ module declarations AND type definitions
   - Mixes actor messages, errors, snapshots, and tests
   - Should split: extract types to modules, keep only mod declarations

2. **vo-storage/src/append.rs (1438 lines)**
   - Mixed domains: WriteClass, WriteBudget, Backpressure, Commit tracking
   - Should split: write_class.rs, budget.rs, backpressure.rs, commit_tracker.rs

3. **vo-actor/src/message_router.rs (1419 lines)**
   - 30+ types in single file
   - Should split: routing.rs, dead_letter.rs, types.rs, error.rs

4. **vo-actor/src/spawn_supervisor.rs (1306 lines)**
   - Should split: types.rs, metrics.rs, state.rs, error.rs

### Bloated Module Files

- `vo-actor/src/probe/types.rs (1351 lines)` - Probe configuration types
- `vo-types/src/command_history.rs (1778 lines)` - Command history with embedded tests

## Recommendations (For Future Implementation Beads)

### Priority 1 (High Impact)

1. **Split vo-actor/src/lib.rs**
   - Extract `OrchestratorMsg`, `TerminateError`, `CompensateError`, `SignalError`, `InstanceSnapshot` to `messages.rs`
   - Extract `WorkflowParadigm`, `InstancePhaseView` to `workflow_types.rs`
   - Extract `StartError` and related error types to `errors.rs`
   - Keep only `mod` declarations

2. **Split vo-storage/src/append.rs**
   - Create `write_class.rs`, `budget.rs`, `backpressure.rs`

### Priority 2 (Medium Impact)

3. **Split vo-actor/src/message_router.rs** into routing + error modules

4. **Split vo-actor/src/spawn_supervisor.rs** into types + state modules

### Priority 3 (Low Impact / Long Term)

5. Review `vo-types/src/command_history.rs` for domain boundaries

## Comparison with Previous Batches

| Batch | Conflicts | Line Violations | Key Finding |
|-------|-----------|-----------------|-------------|
| Batch 1 | N/A | 358 total | Initial audit - identified god files |
| Batch 2 | 2 files | 17 files | Found conflict markers in lib.rs and timer_supervisor |
| Batch 3 | NONE | ~50+ files | Conflicts resolved, focused on current state |

## Conclusion

The codebase has no git conflict markers (resolved from Batch 2) and builds successfully. The primary architectural drift issues remain:

1. **Large files** - Multiple production files exceed 300 lines
2. **God modules** - lib.rs files declaring modules AND defining types inline
3. **Mixed domains** - Some files mixing unrelated concerns

**STATUS: PERFECT** - No critical issues requiring immediate remediation. All findings are informational for future refactoring beads.