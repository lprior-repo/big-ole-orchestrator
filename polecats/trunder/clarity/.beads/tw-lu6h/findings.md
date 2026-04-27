# Architectural Drift Detection Report — wave3-1

**Date:** 2026-04-24
**Bead:** tw-lu6h
**Scope:** Full codebase at /home/lewis/gt/crates/ (312,114 lines across 1,028 files in 15 crates)

---

## Executive Summary

Significant architectural drift detected across three dimensions: file size (29% over limit), ADR compliance (14 violations across 5 ADRs), and cross-crate integrity (8 violations). The most critical findings involve duplicate type definitions that create silent bug potential, missing core infrastructure (DbWriterActor, MasterOrchestrator), and three incompatible WorkloadClass enums.

---

## 1. File Size Drift (300-Line Rule)

**298 of 1,028 files (29%) exceed the 300-line limit.**

### Highest Drift-Rate Crates
| Crate | Drift % | Worst File |
|-------|---------|------------|
| vo-executor | 48.6% | `tests/adr_contract_tests.rs` (1,939) |
| vo-actor | 44.2% | `src/probe.rs` (2,032) |
| vo-cli | 37.1% | `tests/gap_coverage_tests.rs` (2,047) |
| vo-worker | 34.1% | `tests/connector_runtime_contract_tests.rs` (1,742) |
| vo-storage | 33.5% | `src/append.rs` (1,628) |

### Production Source Files Requiring Attention (non-test)
| Lines | File | Multiplier |
|------:|------|-----------|
| 2,032 | `vo-actor/src/probe.rs` | 6.8x |
| 1,914 | `vo-actor/src/lib.rs` | 6.4x |
| 1,628 | `vo-storage/src/append.rs` | 5.4x |
| 1,419 | `vo-types/src/connection_pool/mod.rs` | 4.7x |
| 1,202 | `vo-actor/src/message_router.rs` | 4.0x |
| 1,175 | `vo-actor/src/spawn_supervisor.rs` | 3.9x |
| 1,070 | `vo-storage/src/compensation_saga.rs` | 3.6x |
| 1,075 | `vo-cli/src/commands/doctor_checks.rs` | 3.6x |
| 901 | `vo-core/src/workload_class.rs` | 3.0x |
| 765 | `vo-worker/src/lib.rs` | 2.6x |

### Top 10 Worst Offenders (project-wide)
| Lines | Crate | File |
|------:|-------|------|
| 2,121 | vo-core | `src/replay/red_queen_adversarial_tests.rs` |
| 2,047 | vo-cli | `tests/gap_coverage_tests.rs` |
| 2,032 | vo-actor | `src/probe.rs` |
| 1,939 | vo-executor | `tests/adr_contract_tests.rs` |
| 1,914 | vo-actor | `src/lib.rs` |
| 1,846 | vo-cli | `tests/cli_deep_coverage_tests.rs` |
| 1,742 | vo-worker | `tests/connector_runtime_contract_tests.rs` |
| 1,632 | vo-core | `tests/component_integration.rs` |
| 1,628 | vo-storage | `src/append.rs` |
| 1,611 | vo-cli | `tests/cli_e2e_pipeline_tests.rs` |

**Note:** The majority of oversized files are test files. Of the top 10, only 3 are production source.

---

## 2. ADR Compliance Violations

### ADR-015: Actor Invariants and Backpressure (4 HIGH violations)

| # | Severity | Location | Finding |
|---|----------|----------|---------|
| 1 | HIGH | `vo-actor/src/instance_registry.rs:184-187` | Uses `HashMap` instead of ADR-specified `DashMap`; `!Sync` type cannot be shared across async tasks |
| 2 | HIGH | `vo-actor/src/master.rs:6-8` | `MasterOrchestrator` is empty stub — no lock acquisition, post_stop release, or queuing |
| 3 | HIGH | (not found) | No `DbWriterActor` implementation exists; bounded mailbox requirement entirely unmet |
| 4 | HIGH | (not found) | No 429/503 ingress shedding based on mailbox depth |

**Compliant:** `ExecutionSemaphore` provides bounded concurrency with `BackpressureStatus::ShedLoad`. `InvariantEnforcer` checks single-activity before admission (but is a check, not a lock).

### ADR-018: Pipe Deadlocks and I/O (2 LOW violations)

| # | Severity | Location | Finding |
|---|----------|----------|---------|
| 5 | LOW | `vo-ipc/src/run.rs:152` | Uses `write_all` instead of ADR-prescribed `tokio::io::copy` |
| 6 | LOW | `vo-ipc/src/envelope.rs:51-66` | `write_envelope` uses synchronous `std::io::Write`; latent deadlock risk |

**Compliant:** `perform_ipc` correctly uses `tokio::join!` for concurrent FD3/FD4 I/O. FD3 write end closed via `drop()`. `read_bounded_stderr` has 1MB cap.

### ADR-032: Write-Path QoS (2 MEDIUM violations)

| # | Severity | Location | Finding |
|---|----------|----------|---------|
| 7 | MEDIUM | `vo-core/src/write_class.rs:140-147` | `WriteBudget` uses `RefCell` (not `Sync`), unusable in multi-threaded tokio runtime |
| 8 | MEDIUM | `vo-core/src/workload_budget.rs:8` | Duplicate `WriteBudget` with same `RefCell` problem |

**Compliant:** `WriteClass` taxonomy (3 tiers), `QosRouter` with per-class bounded channels, admission coupling with all 4 pressure indicators.

### ADR-033: Fairness and Workload Classes (2 HIGH violations)

| # | Severity | Location | Finding |
|---|----------|----------|---------|
| 9 | HIGH | `vo-actor/src/fairness.rs:11-15` | Wrong `WorkloadClass` enum — 3 variants (Recovery, NewInstance, Internal) instead of ADR's 4 |
| 10 | HIGH | `vo-core/src/admission/workload.rs:34-45` | Third incompatible `WorkloadClass` — 5 variants (Live, Recovery, TimerResume, NonCritical, Background) |

**Three different WorkloadClass enums exist:**
1. `vo-core/src/workload_class.rs` — Correct (ADR-033, 4 variants)
2. `vo-actor/src/fairness.rs` — Wrong (3 variants)
3. `vo-core/src/admission/workload.rs` — Wrong (5 variants)

**Compliant:** `vo-core/src/workload_class.rs` correctly defines all 4 classes, per-class reserved budgets, `DegradedBudget` blocks non-critical classes.

### ADR-046: Async Process Supervisor (4 violations)

| # | Severity | Location | Finding |
|---|----------|----------|---------|
| 11 | MEDIUM | `vo-actor/src/spawn_supervisor.rs:621` | `transition_phase` method exists but never called; atomic phase transitions not enforced |
| 12 | MEDIUM | `vo-actor/src/spawn_supervisor.rs:678-694` | `spawn_attempts` not incremented on spawn failure path — potential infinite retries |
| 13 | LOW | `vo-actor/src/spawn_supervisor.rs:820-821` | Health check spacing hardcoded 100ms instead of configurable `health_check_interval` |
| 14 | LOW | `vo-actor/src/spawn_supervisor.rs:140` | PID binding invariant not enforced — `spawn_id` can be `Some` in non-Running phases |

**Compliant:** State machine (6 phases), error taxonomy (15 variants, 4 categories), supervisor state machine, all 7 observability metrics, cancellation safety, Send+Sync on all trait objects.

---

## 3. Cross-Crate Architectural Violations

### Rule 1: Actors must not write to fjall directly (MEDIUM)

| Location | Finding |
|----------|---------|
| `vo-api/Cargo.toml:12` + `vo-api/src/handlers/query.rs:22` | vo-api holds `Arc<fjall::Database>` directly, bypassing actor layer (read-path violation) |
| `vo-core/Cargo.toml:24-25` (dev-deps) | Integration tests construct `fjall::Database` directly (test-only) |

**Clean:** vo-actor and vo-executor have zero fjall references.

### Rule 2: No stdout for state (CLEAN)

No violations. All `println!`/`eprintln!` in production code are CLI output, subprocess nullification, or dev diagnostics.

### Rule 3: No Redis/Postgres (CLEAN)

Zero matches across all Cargo.toml files.

### Rule 4: No Wasm in engine (CLEAN)

Zero matches for wasmtime/wasm3/wasmer in engine crates.

### Rule 5: No duplicate types (HIGH — 7 violations)

| # | Type | Locations | Issue |
|---|------|-----------|-------|
| 5a | `InstanceId` | `vo-types/src/string_types.rs` (ULID-validated) vs `vo-common/src/types.rs` (bare `String` alias) | Different validation — common accepts any string including empty |
| 5b | `StepId` | `vo-types/src/string_types.rs` (rejects leading underscore) vs `vo-executor/src/types.rs` (allows leading underscore) | Different validation rules |
| 5c | `TimerId` | `vo-types/src/string_types.rs` (String) vs `vo-types/src/state/semantic_types.rs` (u64) | Same crate, fundamentally different types (String vs integer) |
| 5d | `NodeName` | `vo-types/src/string_types.rs` (validated) vs `vo-types/src/state/semantic_types.rs` (unvalidated) vs `vo-actor/src/signal_messages.rs` (only rejects empty) | Three definitions across two crates |
| 5e | `AttemptNumber` | `vo-types/src/integer_types.rs` (NonZeroU64) vs `vo-types/src/state/semantic_types.rs` (u32) | Different backing types, different sizes |
| 5f | `WorkflowEvent` | `vo-common/src/events.rs` | Core domain type in wrong crate (should be vo-types) |
| 5g | `StepResult` | `vo-executor/src/types.rs` | Core domain type in wrong crate (should be vo-types) |

---

## 4. Prioritized Recommendations

### CRITICAL (silent bug potential)
1. Resolve internal `vo-types` type conflicts (TimerId String vs u64, AttemptNumber NonZeroU64 vs u32, NodeName 3x)
2. Consolidate duplicate `WorkloadClass` enums — single source of truth in `vo-core/src/workload_class.rs`

### HIGH (missing infrastructure)
3. Implement `DbWriterActor` with bounded mailbox (ADR-015)
4. Implement `MasterOrchestrator` with DashMap-based registry (ADR-015)
5. Add 429/503 ingress shedding based on mailbox depth (ADR-015)
6. Move `vo-common::InstanceId` and `vo-common::WorkflowEvent` to `vo-types`
7. Move `vo-executor::StepId` and `vo-executor::StepResult` to `vo-types`

### MEDIUM (correctness concerns)
8. Replace `RefCell` with `AtomicU64` in `WriteBudget`/`WorkloadBudget` for Sync safety
9. Use `SpawnStorage::transition_phase` for atomic phase transitions (ADR-046)
10. Increment `spawn_attempts` on failure path (ADR-046)
11. Refactor `vo-api` query handlers to use `vo-storage` abstraction instead of `Arc<fjall::Database>`

### LOW (cosmetic / test)
12. Replace `HashMap` with `DashMap` in InstanceRegistry
13. Fix hardcoded 100ms health check interval
14. Enforce PID binding invariant in spawn_supervisor
15. Split oversized production source files (probe.rs, lib.rs, append.rs, etc.)

---

## Methodology

- **File size:** `wc -l` on all .rs files excluding target/, checked against 300-line limit
- **ADR compliance:** Read ADR text, grepped source code for compliance patterns, verified specific line references
- **Cross-crate:** Grep for fjall/println!/redis/postgres/wasmtime dependencies; traced type definitions via grep and source reading
- **Scope:** 15 crates, 312,114 lines, 1,028 files
