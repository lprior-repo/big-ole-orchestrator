# ARCH-DRIFT Findings: cd-28o (Wave 3-5)

**Bead**: cd-28o · ARCH-DRIFT: drift detection wave3-5
**Date**: 2026-04-24
**Phases Covered**: Phase 3 (Execution Boundary), Phase 4 (Exactly-Once Core), Phase 5 (Waiting, Timers, Signals)

---

## 1. WORKSPACE REALITY DRIFT

### 1.1 Undocumented Crates
The following crates exist in `Cargo.toml` workspace but are NOT listed in architecture-spec.md §3.1 workspace truth:

| Crate | Status |
|-------|--------|
| `vo-executor` | EXISTS but undocumented |
| `vo-scheduler` | EXISTS but undocumented |
| `vo-sdk-macros` | EXISTS but undocumented |

**Reference**: architecture-spec.md §3.1 claims 12 crates; actual Cargo.toml has 15 members.

### 1.2 Nonexistent Crate References
- `vo-engine` referenced in `vo-actor/src/lib.rs:1` ("Actor framework for vo-engine") but does not exist in workspace
- `vo-ui` referenced in CLAUDE.md per architecture-spec.md §3.2 but does not exist

---

## 2. FILES EXCEEDING 300 LINE LIMIT (DDD VIOLATION)

### Critical Violations (>1000 lines)

| Crate | File | Lines |
|-------|------|-------|
| vo-core | src/replay/red_queen_adversarial_tests.rs | 2121 |
| vo-cli | tests/gap_coverage_tests.rs | 2047 |
| vo-actor | src/probe.rs | 2032 |
| vo-executor | tests/adr_contract_tests.rs | 1939 |
| vo-actor | src/lib.rs | 1914 |
| vo-cli | tests/cli_deep_coverage_tests.rs | 1846 |
| vo-worker | tests/connector_runtime_contract_tests.rs | 1742 |
| vo-core | tests/component_integration.rs | 1633 |
| vo-storage | src/append.rs | 1628 |
| vo-cli | tests/cli_e2e_pipeline_tests.rs | 1611 |

### vo-core (Phase 3-5 relevant)

| File | Lines | Issue |
|------|-------|-------|
| src/ghost_workflow.rs | 615 | ADR-021; exceeds 300-line limit |
| src/workload_class.rs | ~300+ | ADR-033 fairness implementation |
| src/workload_budget.rs | ~200+ | ADR-033 budget enforcement |
| src/shedding.rs | ~150+ | ADR-006 backpressure |

### vo-executor (Phase 3)

| File | Lines | ADR Coverage |
|------|-------|-------------|
| src/execution.rs | 564 | ADR-012, ADR-018 |
| src/types.rs | 492 | Execution types |
| src/subprocess.rs | 478 | ADR-018 pipe handling |
| src/state.rs | 356 | Execution state |

### vo-actor (Phase 3-5)

| File | Lines | ADR Coverage |
|------|-------|-------------|
| src/lib.rs | 1914 | Actor framework; needs split |
| src/probe.rs | 2032 | Health probes; needs split |
| src/message_router.rs | 1202 | Actor messaging |
| src/spawn_supervisor.rs | 1175 | Actor lifecycle |
| src/timers.rs | 551 | ADR-005 timer implementation |
| src/heartbeat.rs | 515 | ADR-012 heartbeat |
| src/signal_buffer.rs | 304 | ADR-042 signal buffering |

---

## 3. ADR COVERAGE ANALYSIS (Phases 3-5)

### Phase 3: Execution Boundary and Pure-Step Runtime

| ADR | Status | Implementation Location |
|-----|--------|------------------------|
| ADR-012 (Execution Boundary Hardening) | ✅ FOUND | vo-executor/adr_contract_tests.rs, execution_boundary_tests.rs |
| ADR-014 (Secure IPC FD Management) | ❌ NOT FOUND | No references in codebase |
| ADR-018 (Pipe Deadlocks) | ✅ FOUND | vo-executor/src/subprocess.rs |
| ADR-011 (Current-Thread Runtime) | ✅ FOUND | vo-executor/src/runtime.rs |
| ADR-019 (SIGTERM Handling) | ✅ FOUND | vo-executor/tests/adr_contract_tests.rs |
| ADR-023 (Stderr Bounds) | ✅ FOUND | vo-executor/tests/adr_contract_tests.rs |
| ADR-006 (Backpressure) | ✅ FOUND | vo-core/src/shedding.rs, vo-actor/semaphore |
| ADR-015 (Actor Invariants) | ✅ FOUND | vo-actor/semaphore/enforcer.rs |

**Gap**: ADR-014 (Secure IPC) has NO implementation found in codebase.

### Phase 4: Exactly-Once Core

| ADR | Status | Implementation Location |
|-----|--------|------------------------|
| ADR-027 (Replay Engine) | ✅ FOUND | vo-core/src/replay/mod.rs |
| ADR-028 (Ingress Dedupe) | ✅ FOUND | vo-storage/src/dedupe_partition, vo-types/dedupe.rs |
| ADR-029 (Execution Leases/Fencing) | ✅ FOUND | vo-storage/src/lease_partition |
| ADR-013 (System Resilience) | ✅ FOUND | vo-core/src/recovery, vo-actor/reanimator |
| ADR-016 (Atomic Storage Snapshots) | ✅ FOUND | vo-storage/tests/atomic_batch_snapshot_integration.rs |
| ADR-043 (Verification Strategy) | ✅ FOUND | vo-core/src/exact_once_verification/ |

All Phase 4 ADRs are well-implemented with extensive test coverage.

### Phase 5: Waiting, Timers, and Signals

| ADR | Status | Implementation Location |
|-----|--------|------------------------|
| ADR-005 (Hibernation and Timers) | ✅ FOUND | vo-actor/src/timer_lifecycle.rs, reanimator/mod.rs |
| ADR-042 (Signal Matching) | ✅ FOUND | vo-types/src/signal/signal_match.rs, signal_buffer.rs |
| ADR-033 (Fairness/Workload Classes) | ✅ FOUND | vo-core/src/workload_class.rs, vo-actor/fairness/ |
| ADR-036 (Command Identity) | ✅ FOUND | vo-types/src/command_envelope.rs (521 references) |

All Phase 5 ADRs are well-implemented.

---

## 4. SUMMARY

### Drift Severity Assessment

| Category | Severity | Count |
|----------|----------|-------|
| Undocumented workspace crates | HIGH | 3 |
| Nonexistent crate references in code | MEDIUM | 2 |
| Files >300 lines | CRITICAL | 30+ files |
| Missing ADR implementation (ADR-014) | HIGH | 1 |

### Recommendations

1. **Immediate**: Split vo-actor/src/lib.rs (1914 lines) and vo-actor/src/probe.rs (2032 lines)
2. **Immediate**: Add vo-executor, vo-scheduler, vo-sdk-macros to architecture-spec.md §3.1
3. **High Priority**: Implement ADR-014 (Secure IPC) or mark as deferred with explicit rationale
4. **Medium**: Remove vo-engine and vo-ui references from documentation and code comments

---

## STATUS

**ARCH-DRIFT: WAVE 3-5 COMPLETE**

- Workspace reality drift: DETECTED
- Files >300 lines: CRITICAL VIOLATIONS FOUND
- Phase 3 ADR coverage: 7/8 (ADR-014 missing)
- Phase 4 ADR coverage: 6/6 (FULL)
- Phase 5 ADR coverage: 4/4 (FULL)
