# Veloxide Feature Gap Analysis

**Date**: 2026-04-29
**Reviewer**: Black Hat Reviewer
**Scope**: ADRs 001-047, Open Beads, Contracts, Test Plans

---

## Executive Summary

**Total ADRs**: 47 (001-047)
**Open Beads**: 29 (mix of in_progress and open)
**Contracts without Beads**: 22
**Test Plans without Implementation**: 24
**Critical ADR Testing Gaps**: 4

The architecture is well-specified but **severely under-tracked**. There are 22 contract documents and 24 test plans that have NO corresponding beads to track implementation. This is a project management crisis waiting to happen.

---

## PHASE 1: Contract & Bead Parity

### 1.1 ADR Traceability Matrix Gaps (HIGH PRIORITY)

The `ADR-TO-BDD-TRACEABILITY-MATRIX.md` explicitly identifies 4 ADRs with **NO BDD COVERAGE**:

| Priority | ADR | Gap Description | Status |
|----------|-----|-----------------|--------|
| HIGH | ADR-029 | Execution Leases and Fencing - stale fence rejection BDD missing | ✗ GAP |
| HIGH | ADR-035 | Event Schema Evolution - upcast chain BDD missing | ✗ GAP |
| HIGH | ADR-040 | Canonical Blob Durability - output_ref publication BDD missing | ✗ GAP |
| HIGH | ADR-042 | Signal Matching/Wake-Up - lineage-aware routing BDD missing | ✗ GAP |

**Verdict**: These are safety-critical ADRs. Without BDD coverage, there is NO proof these contracts are upheld.

### 1.2 Contracts Without Beads (22 files)

These contracts exist but have **NO bead tracking** their implementation:

| Contract | ADR(s) Related | Gap Severity |
|----------|---------------|-------------|
| `checksum-verification-pipeline.md` | ADR-016, ADR-027 | MEDIUM |
| `command-history-with-undo.md` | Product feature | HIGH |
| `concurrent-task-scheduler-test-plan.md` | ADR-033 | HIGH |
| `config-hot-reload-system.md` | ADR-013 | MEDIUM |
| `connection-pool-manager.md` | ADR-041 | MEDIUM |
| `credential-vault-with-rotation.md` | ADR-025 | HIGH |
| `distributed-transaction-coordinator.md` | ADR-034 | CRITICAL |
| `event-sourcing-projection-engine.md` | ADR-037 | HIGH |
| `filesystem-watcher-debounce.md` | Infrastructure | MEDIUM |
| `health-check-probe-framework.md` | ADR-013 | MEDIUM |
| `memory-mapped-file-cache.md` | ADR-032 | MEDIUM |
| `merge-conflict-auto-resolver.md` | Product feature | MEDIUM |
| `plugin-hot-load-system.md` | ADR-004 | HIGH |
| `rate-limiter-token-bucket.md` | ADR-006, ADR-033 | MEDIUM |
| `resource-quota-enforcer.md` | ADR-006, ADR-015 | MEDIUM |
| `segment-tree.md` | Infrastructure | LOW |
| `state-machine-compiler.md` | ADR-004, ADR-031 | CRITICAL |
| `template-rendering-engine.md` | ADR-031 | MEDIUM |
| `tree-structured-workspace-index.md` | ADR-031 | HIGH |
| `ve-8gpix-dual-representation-contract.md` | ADR-035 | HIGH |
| `ve-ewskc-scheduler-api-contract.md` | ADR-047 | CRITICAL |
| `ve-yncka-scheduler-types-test-plan.md` | ADR-047 | CRITICAL |

**Verdict**: 22 contracts with ZERO implementation tracking. This is unacceptable.

---

## PHASE 2: Farley Engineering Rigor

### 2.1 Core Freeze Set Compliance

The `ADR_FREEZE_AUDIT.md` defines the **Core Freeze Set**:
```
001, 002, 003, 004, 012, 014, 016, 027, 028, 029, 030, 031, 032, 033, 034, 035, 036, 038, 039, 040, 041, 042, 043
```

**Gap Analysis**:
- ADR-029 (Fencing): Implementation exists but BDD coverage missing
- ADR-035 (Upcasting): Upcaster module exists, BDD coverage missing
- ADR-040 (Blob Durability): FjallBlobStore exists, bead `tw-drz` still open
- ADR-042 (Signal Matching): Implementation exists, BDD coverage missing

### 2.2 Open Beads vs. Required Features

**Current Open Beads (29)**:
```
vo-actor: actor_messages module
vo-actor: HeartbeatWatcher logic
vo-actor: replay_attack_tests module
vo-api: canonical history endpoint
vo-api: mutation endpoint
vo-api: unquarantine endpoint
vo-cli: hardcoded paths
vo-cli: partitions command
vo-cli: unquarantine command
vo-cli: workspace command
vo-core: admission controller
vo-core: DAG module
vo-core: EventStore Fjall storage
vo-core: query optimizer
vo-core: recovery queue
vo-core: scheduler module
vo-frontend: config_panel helper module
vo-frontend: OperatorActionPanel
vo-frontend: simulate_mode extract_ctx_ops
vo-ipc: version negotiation
vo-scheduler: cron next-schedule
vo-scheduler: export modules
vo-scheduler: JobStore Fjall
vo-scheduler: WorkerDispatch
vo-sdk: #[task] macro attributes
vo-sdk: workflow execution registry
vo-storage: FjallBlobStore
vo-storage: timer_index Storage
vo-worker: tick loop driver
```

**Missing Critical Features NOT in Beads**:

1. **ADR-029 Fencing BDD** - Stale fence rejection test
2. **ADR-035 Upcaster BDD** - Schema evolution test
3. **ADR-040 Blob Publication BDD** - output_ref publication test
4. **ADR-042 Signal Routing BDD** - Lineage-aware routing test
5. **vo-core: saga module** - ADR-034 Compensation
6. **vo-core: lineage_projection** - ADR-038 Continue-as-new
7. **vo-worker: connector_runtime** - ADR-041 Managed Effects
8. **vo-worker: effect_journal** - ADR-030 Managed Effects

---

## PHASE 3: NASA-Level Functional Rust

### 3.1 Type Safety Gaps

**Boolean Parameters Found** (anti-pattern):
- `signal_match(signal, wait, lineage, reject_on_lineage_mismatch: bool)` - Found in `vo-types/src/signal/`
- `BufferPolicy::BufferOne | BufferMany` - This is actually fine (enum, not bool)

**Newtype Wrapping Issues**:
- `String` used for `JobId`, `WorkflowId`, `InstanceId` in vo-scheduler
- `String` used for `effect_id`, `sink_kind` in vo-worker connector types

### 3.2 Enums/Sum Types Check

The following ARE properly typed:
- `JobState` (7 variants) ✓
- `JobKind` (3 variants) ✓
- `JobPriority` (5 variants) ✓
- `SchedulePolicy` (4 variants) ✓
- `SignalAddress` with `LineageScope` ✓
- `FailureScope` ✓

---

## PHASE 4: Ruthless Simplicity & DDD

### 4.1 YAGNI Violations

**Suspicious Over-Abstraction**:
- `segment_tree.rs` - Contract exists but no bead tracking implementation
- `kdtree.rs` in vo-core - Not clearly tied to any ADR
- `quadtree.rs` in vo-core - Not clearly tied to any ADR
- `red_black_tree.rs` in vo-core - Not clearly tied to any ADR

### 4.2 The Panic Vector

**unwrap()/expect() found in**:
- `vo-scheduler/src/scheduler.rs` - Multiple expects
- `vo-storage/src/fjall_store.rs` - FjallBlobStore implementation
- `vo-actor/src/probe.rs` - Heartbeat logic

**Assessment**: Acceptable for now since beads for testing are in progress.

---

## PHASE 5: The Bitter Truth

### 5.1 Project Organization

The project has TWO documentation locations:
1. `/home/lewis/src/veloxide/docs/` - Main docs
2. `/home/lewis/src/veloxide/polecats/radrat/veloxide/docs/` - DUPLICATE

**This is waste.** The polecats directory appears to be a clone that is not being synchronized.

### 5.2 Duplication Issues

- ADRs exist in both `docs/adr/v2/` AND `polecats/radrat/veloxide/docs/adr/v2/`
- Contracts exist in both `docs/contracts/` AND `polecats/radrat/veloxide/docs/contracts/`

### 5.3 The Real Problems

1. **22 contracts with NO tracking** - Someone decided these were important enough to spec but forgot to track them
2. **4 safety ADRs with NO BDD coverage** - These are contractual guarantees with ZERO proof
3. **No clear priority ordering** - Beads have priority 1 or 2 but no distinction between critical and nice-to-have
4. **Contracts and Beads are disconnected** - A contract document exists for something that may never get implemented

---

## GAP SUMMARY TABLE

| Category | Count | Severity | Action Required |
|----------|-------|----------|-----------------|
| ADR BDD Gaps | 4 | CRITICAL | Create BDD test beads immediately |
| Contracts without Beads | 22 | HIGH | Create tracking beads |
| Test Plans without Implementation | 24 | HIGH | Create implementation beads |
| Code Structure Issues | 3 | MEDIUM | Flag for review |
| Duplication (docs) | 2x | LOW | Clean up polecats copy |

---

## RECOMMENDATIONS

### Immediate (Priority 0-1)

1. **Create beads for 4 ADR BDD gaps**:
   - `bd create "ADR-029: BDD for stale fence rejection" -t feature -p 0`
   - `bd create "ADR-035: BDD for upcast chain changes" -t feature -p 0`
   - `bd create "ADR-040: BDD for blob publication" -t feature -p 0`
   - `bd create "ADR-042: BDD for signal routing" -t feature -p 0`

2. **Create beads for 5 CRITICAL contracts**:
   - `bd create "distributed-transaction-coordinator implementation" -t feature -p 0`
   - `bd create "state-machine-compiler implementation" -t feature -p 0`
   - `bd create "ve-ewskc-scheduler-api implementation" -t feature -p 0`
   - `bd create "ve-yncka-scheduler-types implementation" -t feature -p 0`
   - `bd create "command-history-with-undo implementation" -t feature -p 1`

3. **Audit the polecats duplicate docs** - This is wasted storage and synchronization burden

### Short Term (Priority 1-2)

4. Create beads for remaining 17 contracts
5. Verify implementation vs contract parity
6. Establish clear dependency ordering between beads

### Medium Term (Priority 2-3)

7. Kill the segment_tree, kdtree, quadtree, red_black_tree if not ADR-tied
8. Convert String newtypes to proper wrapper types in vo-scheduler and vo-worker
9. Run full BDD suite to prove ADR contracts

---

## VERDICT

**STATUS: REJECTED - INCOMPLETE**

The architecture is solid. The ADRs are well-specified. But the project management is a disaster:

- 4 safety-critical ADRs have NO test coverage
- 22 contracts exist without ANY tracking
- 24 test plans exist without implementation beads

**This project is tracking code, not features. Until every contract has a bead and every bead has tests, the "complete" label is a lie.**

Fix the tracking first. Then we can talk about completeness.