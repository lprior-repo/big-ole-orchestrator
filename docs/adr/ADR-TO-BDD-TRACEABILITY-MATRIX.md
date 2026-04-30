# ADR to BDD Traceability Matrix

## Overview

This document maps every **safety ADR** (Architecture Decision Record) to its corresponding BDD test coverage,
requirement IDs, bead IDs, and proof commands.

**Definition**: Safety ADRs are correctness-critical ADRs that define exact-once semantics, invariants,
or contractual guarantees where failure could result in data loss, corruption, or violation of
durability guarantees.

---

## Safety ADR Coverage Matrix

| ADR | Title | BDD Test File(s) | Requirement IDs | Bead IDs | Test Count | Status |
|-----|-------|------------------|-----------------|----------|------------|--------|
| **ADR-026** | AI Loop Poisoning Circuit Breakers | `bdd_circuit_breaker_quarantine.rs` | INV-026-001, INV-026-002 | tw-xxx | 3 | ✓ COVERED |
| **ADR-027** | Deterministic Event-Sourced Replay | `bdd_workflow_version_replay.rs`, `proptest_replay.rs` | INV-027-001 through INV-027-008 | tw-xxx | 12+ | ✓ COVERED |
| **ADR-028** | Exactly-Once Ingress Deduplication | `bdd_command_dedup.rs`, `bdd_mutation_dedupe.rs` | INV-028-001 through INV-028-004 | tw-xxx | 6 | ✓ COVERED |
| **ADR-029** | Execution Leases and Fencing | _(see note)_ | INV-029-001 through INV-029-003 | tw-xxx | 0 | ✗ GAP |
| **ADR-034** | Saga Compensation and Reversibility | `bdd_compensating_completion_rejection.rs` | INV-034-001 through INV-034-004 | tw-xxx | 4+ | ✓ COVERED |
| **ADR-035** | Event Schema Evolution and Upcasting | _(see note)_ | INV-035-001 through INV-035-003 | tw-xxx | 0 | ✗ GAP |
| **ADR-036** | Command Identity, Correlation, Causation | `bdd_command_dedup.rs`, `bdd_mutation_dedupe.rs` | INV-036-001 through INV-036-003 | tw-xxx | 6 | ✓ COVERED |
| **ADR-039** | Hierarchical Lifecycle State Machine | `bdd_compensating_completion_rejection.rs` | INV-039-001 through INV-039-005 | tw-xxx | 4+ | ✓ COVERED |
| **ADR-040** | Canonical Blob Durability and Publication | _(see note)_ | INV-040-001 through INV-040-003 | tw-xxx | 0 | ✗ GAP |
| **ADR-041** | Managed Connector Runtime Contract | `connector_runtime_contract_tests.rs` (vo-worker) | INV-041-001 through INV-041-006 | tw-xxx | 20+ | ✓ COVERED |
| **ADR-042** | Signal Matching and Wake-Up Semantics | `vo-types/adr042_signal_wakeup_bdd.rs`, `vo-core/lineage_signals.rs` | INV-042-001 through INV-042-004 | tw-xxx | 15+ | ✓ COVERED |
| **ADR-043** | Exact-Once Verification Strategy | `proptest_replay.rs` | INV-043-001 through INV-043-007 | tw-xxx | 8+ | ✓ COVERED |

---

## Detailed Coverage Analysis

### ADR-026: AI Loop Poisoning Circuit Breakers
- **Test File**: `crates/vo-core/tests/bdd_circuit_breaker_quarantine.rs`
- **Tests**: 3 BDD scenarios
- **Invariant Coverage**:
  - INV-026-001: Quarantine threshold triggers after consecutive failures
  - INV-026-002: Quarantine prevents dispatch to poisoned AI loop

### ADR-027: Deterministic Event-Sourced Replay
- **Test Files**:
  - `crates/vo-core/tests/bdd_workflow_version_replay.rs` (BDD scenarios)
  - `crates/vo-core/tests/proptest_replay.rs` (Property-based tests)
- **Tests**: 12+ scenarios + property tests
- **Invariant Coverage**:
  - INV-027-001: Replay produces identical state for same event sequence
  - INV-027-002: Version normalization happens before apply()
  - INV-027-003: Snapshots are discarded if upcast fails

### ADR-028: Exactly-Once Ingress Deduplication
- **Test Files**:
  - `crates/vo-core/tests/bdd_command_dedup.rs`
  - `crates/vo-core/tests/bdd_mutation_dedupe.rs`
- **Tests**: 6 BDD scenarios
- **Invariant Coverage**:
  - INV-028-001: Duplicate command_id returns original outcome
  - INV-028-002: First submission admitted, tracked as duplicate thereafter
  - INV-028-003: Different command_ids both admitted
  - INV-028-004: No prior submission returns false for duplicate check

### ADR-029: Execution Leases and Fencing
- **Status**: ✗ NO DIRECT BDD COVERAGE
- **Note**: Fence validation is embedded in `DbWriterActor` commit path. Integration coverage via `red_queen_adversarial` tests.
- **Gap**: Needs dedicated BDD scenario for stale fence rejection

### ADR-034: Saga Compensation and Reversibility
- **Test File**: `crates/vo-core/tests/bdd_compensating_completion_rejection.rs`
- **Tests**: 4+ BDD scenarios
- **Invariant Coverage**:
  - INV-034-001: Normal completion rejected while compensating
  - INV-034-002: Compensation state transitions are explicit
  - INV-034-003: Compensation follows reverse dependency order

### ADR-035: Event Schema Evolution and Upcasting
- **Status**: ✗ NO DIRECT BDD COVERAGE
- **Note**: Upcaster integration tests exist in `upcaster_integration.rs` and `upcaster_proptest.rs`
- **Gap**: Needs BDD scenario proving old events replay correctly after upcast chain changes

### ADR-036: Command Identity, Correlation, Causation
- **Test Files**: Same as ADR-028
- **Invariant Coverage**:
  - INV-036-001: command_id enables idempotent retries
  - INV-036-002: correlation_id groups business flow
  - INV-036-003: causation_id traces immediate parent

### ADR-039: Hierarchical Lifecycle State Machine
- **Test File**: `crates/vo-core/tests/bdd_compensating_completion_rejection.rs`
- **Invariant Coverage**:
  - INV-039-001: States are mutually exclusive
  - INV-039-002: Compensation state blocks normal completion
  - INV-039-003: State transitions are atomic

### ADR-040: Canonical Blob Durability and Publication
- **Status**: ✗ NO DIRECT BDD COVERAGE
- **Gap**: Needs BDD scenario proving output_ref is never published before blob durability

### ADR-041: Managed Connector Runtime Contract
- **Test File**: `crates/vo-worker/tests/connector_runtime_contract_tests.rs`
- **Tests**: 20+ scenarios
- **Invariant Coverage**:
  - INV-041-001: prepare → commit → reconcile lifecycle
  - INV-041-002: Timeout does not mean failure
  - INV-041-003: Reconciliation before retry on ambiguity
  - INV-041-004: Durable receipts for audit

### ADR-042: Signal Matching and Wake-Up Semantics
- **Status**: ✓ COVERED
- **Test File**: `vo-types/tests/adr042_signal_wakeup_bdd.rs` (unit tests), `vo-core/tests/lineage_signals.rs` (integration tests)
- **Tests**: 15+ BDD scenarios covering lineage-aware signal routing, wait-state matching, epoch-local vs lineage-wide routing, and rollover semantics
- **Invariant Coverage**:
  - INV-042-001: Signal routing based on lineage scope
  - INV-042-002: Wait state matching to signal patterns
  - INV-042-003: Signal delivered to correct lineage OR queued
  - INV-042-004: Signal never delivered to mismatched lineage

### ADR-043: Exact-Once Verification Strategy
- **Test File**: `crates/vo-core/tests/proptest_replay.rs`
- **Tests**: 8+ property-based invariants
- **Invariant Coverage**:
  - INV-043-001: Duplicate ingress does not create duplicate work
  - INV-043-002: Replay after crash reaches same legal state
  - INV-043-003: Lineage rollover preserves signal routing

---

## Gap Summary

| Priority | ADR | Gap Description |
|----------|-----|-----------------|
| HIGH | ADR-029 | Needs BDD for stale fence rejection at DbWriterActor |
| HIGH | ADR-035 | Needs BDD for upcast chain changes and old event replay |
| HIGH | ADR-040 | Needs BDD for output_ref publication after blob durability |
| MEDIUM | ADR-042 | Covered by vo-types adr042_signal_wakeup_bdd.rs + vo-core lineage_signals.rs |

---

## Proof Commands

```bash
# Run all BDD tests for safety ADRs
cargo test -p vo-core given_adr_traceability_when_checked_then_each_safety_adr_has_bdd_coverage

# Run ADR-026 circuit breaker BDD tests
cargo test -p vo-core bdd_circuit_breaker_quarantine

# Run ADR-027/ADR-043 replay property tests
cargo test -p vo-core proptest_replay

# Run ADR-028/ADR-036 deduplication BDD tests
cargo test -p vo-core bdd_command_dedup
cargo test -p vo-core bdd_mutation_dedupe

# Run ADR-034/ADR-039 compensation BDD tests
cargo test -p vo-core bdd_compensating_completion_rejection

# Run ADR-041 connector contract tests
cargo test -p vo-worker connector_runtime_contract

# Run ADR-042 signal matching tests
cargo test -p vo-types --test adr042_signal_wakeup_bdd
cargo test -p vo-core lineage_signals

# Run all red-queen adversarial tests (covers multiple ADRs)
cargo test -p vo-core red_queen_adversarial
```

---

## Maintenance

This matrix is automatically verified by the `given_adr_traceability_when_checked_then_each_safety_adr_has_bdd_coverage`
BDD test in `crates/vo-core/tests/bdd_adr_traceability_matrix.rs`.

When adding new safety ADRs:
1. Add the ADR to this matrix
2. Create corresponding BDD test(s)
3. Update the verification test to include the new ADR
4. Ensure `cargo test -p vo-core given_adr_traceability_when_checked_then_each_safety_adr_has_bdd_coverage` passes