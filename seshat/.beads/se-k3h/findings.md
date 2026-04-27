# Architectural Drift Audit - Batch 1 Findings

## Bead: se-k3h
**Title:** ARCH-DRIFT: architectural drift analysis and refactoring - batch 1
**Status:** Completed by ghoul
**Date:** 2026-04-24

---

## Methodology

Audited all Rust files across 14 crates at `/home/lewis/gt/crates/` using line counts and structural analysis.

---

## Key Metrics

- **358** total files exceed 300-line limit (tests + production)
- **171** production files exceed 300-line limit
- **312,282** total lines across all Rust files

---

## Top 10 Production Violators

| File | Lines | Issue |
|------|-------|-------|
| vo-actor/src/probe.rs | 2032 | 15 types inline |
| vo-actor/src/lib.rs | 1914 | 40+ declarations + inline types |
| vo-storage/src/append.rs | 1628 | 37 types, 5 mixed domains |
| vo-types/src/connection_pool/mod.rs | 1419 | 23 types |
| vo-types/src/cartesian_tree.rs | 1302 | 65% tests |
| vo-actor/src/message_router.rs | 1202 | 30 types |
| vo-actor/src/spawn_supervisor.rs | 1175 | 18 types |
| vo-types/src/btree.rs | 1143 | 44% tests |
| vo-cli/src/commands/doctor_checks.rs | 1075 | 8 types |
| vo-storage/src/compensation_saga.rs | 1070 | 18 types |

---

## Primitive Obsession

- Raw `String` for: projection_id, blob_id, effect_id, workflow_name, binary_name, write_key
- Raw `u32` for PID in ProcessHandle
- Raw `i64` for TimestampMs
- Inconsistent: some IDs wrapped (ChannelId, PoolId, ConnectionId), others raw

---

## Structural Patterns

1. **God files:** probe.rs, append.rs, message_router.rs contain entire subsystems
2. **Mixed domains:** append.rs mixes budget+queue+backpressure+commit+entries
3. **Test inflation:** cartesian_tree.rs 65% tests, btree.rs 44% tests
4. **Bloated lib.rs:** vo-actor/src/lib.rs declares modules AND defines types

---

## Recommended Splits

### probe.rs → 
- config/, probes/(http,tcp,exec), registry.rs, error.rs, types.rs

### lib.rs (vo-actor) →
- Extract inline types to modules, keep mod declarations only

### append.rs →
- write_class.rs, budget.rs, queue.rs, backpressure.rs, commit_tracker.rs, entry.rs

### connection_pool/mod.rs →
- config.rs, types.rs, error.rs, stats.rs

### cartesian_tree.rs →
- Extract tests to tests/ module

### message_router.rs →
- types.rs, dead_letter.rs, routing.rs, error.rs

### spawn_supervisor.rs →
- types.rs, metrics.rs, state.rs, error.rs

### btree.rs →
- Extract tests to tests/ module

### doctor_checks.rs →
- types.rs + per-category check modules

### compensation_saga.rs →
- types.rs, reconciler.rs, manifest.rs, error.rs

---

## Audit-Only Note

This bead was audited from seshat polecat worktree (no source code access). Actual refactoring requires implementation beads filed against the veloxide repo.

---

## Completion

- **Auditor:** seshat/polecats/ghoul
- **Completed:** 2026-04-24
- **Code Changes:** None (audit-only)