# cl-izf Findings: Plan Improvements for Clarity (Veloxide)

**Date:** 2026-04-24
**Polecat:** lancer (rig: clarity)
**Bead Status:** Phantom (cl-izf does not exist in Dolt DB — dispatch error)

---

## Executive Summary

Veloxide is a 15-crate Rust workspace (~210K lines, ~13K tests) implementing a single-binary FaaS orchestrator with durable execution, ractor actors, and fjall persistence. The codebase has strong test coverage and a mature ADR corpus (45 docs), but suffers from significant architectural drift.

**Key metrics:**
- 151 source files exceed the 300-line limit
- 6 crates missing workspace lint adoption
- ~7,446 unwrap/expect calls in src/ (many in test code)
- Documentation drift (old "vo-engine" references, stub vision doc)

---

## P0 — Critical Issues

### 1. 151 Files Exceed 300-Line Limit

The worst offenders:

| File | Lines | Crate |
|------|-------|-------|
| vo-actor/src/probe.rs | 2,032 | vo-actor |
| vo-actor/src/lib.rs | 1,914 | vo-actor |
| vo-core/src/replay/red_queen_adversarial_tests.rs | 2,121 | vo-core |
| vo-storage/src/append.rs | 1,628 | vo-storage |
| vo-types/src/workflow_tests.rs | 1,607 | vo-types |
| vo-types/src/connection_pool/mod.rs | 1,419 | vo-types |
| vo-types/src/tx_coordinator/tests.rs | 1,394 | vo-types |
| vo-core/src/invalid_business_data_tests.rs | 1,215 | vo-core |
| vo-actor/src/message_router.rs | 1,202 | vo-actor |
| vo-actor/src/spawn_supervisor.rs | 1,175 | vo-actor |
| vo-cli/src/commands/doctor_checks.rs | 1,075 | vo-cli |

**Recommendation:** Split the top-20 largest files first. Focus on vo-actor (probe.rs, lib.rs, message_router.rs, spawn_supervisor.rs) and vo-core (replay tests).

### 2. vo-actor Crate Root is 1,914 Lines

`lib.rs` contains lock manager implementation and types inline. Should be extracted into separate modules.

---

## P1 — High Priority

### 3. 6 Crates Missing Workspace Lint Adoption

These crates do NOT have `[lints] workspace = true`:
- **vo-api** — no lints at all
- **vo-core** — no lints at all
- **vo-scheduler** — partial (only `lints.rust`)
- **vo-sdk** — no lints at all
- **vo-sdk-macros** — no lints at all
- (vo-executor — has lints but unverified)

**Fix:** Add to each crate's Cargo.toml:
```toml
[lints]
workspace = true
```

### 4. Unwrap/Expect Debt (~7,446 calls)

| Crate | unwrap | expect | Total |
|-------|--------|--------|-------|
| vo-storage | 1,987 | 389 | 2,376 |
| vo-types | 1,520 | 703 | 2,223 |
| vo-core | 970 | 361 | 1,331 |
| vo-actor | 466 | 141 | 607 |
| vo-sdk | 130 | 381 | 511 |

Many are in `#[cfg(test)]` (acceptable), but production code paths need audit. Start with vo-storage and vo-types.

### 5. Documentation Drift

| Issue | Location |
|-------|----------|
| README.md title says "vo-engine" | `/README.md:1` |
| docs/VISION_AND_ARCHITECTURE.md is 1-line stub | `/docs/VISION_AND_ARCHITECTURE.md` |
| lib.rs doc comments reference "vo-engine" | vo-actor, vo-api, vo-core |
| architecture-spec.md lists 12 crates | Workspace has 15 |
| ADR numbering gaps | ADR-044, ADR-045 missing |

### 6. Suppressed Warnings

- vo-worker: `#![allow(unused)]`, `#![allow(missing_docs)]`
- vo-sdk: `#![allow(unexpected_cfgs)]`
- vo-sdk-macros: `#![allow(dead_code, unused_variables)]`

---

## P2 — Medium Priority

### 7. Dependency Version Inconsistency

vo-cli and vo-sdk hardcode dependency versions instead of using workspace references:

| Crate | Hardcoded deps |
|-------|----------------|
| vo-cli | `clap`, `sha2`, `toml`, `libc`, version, edition |
| vo-sdk | `serde`, `serde_json`, `libc` |
| vo-ipc | `libc` |

### 8. Moon Build Config Gaps

Only 5 of 15 crates have per-crate `moon.yml`:
- vo-worker, vo-types, vo-ipc, vo-cli, vo-api

Missing: vo-core, vo-actor, vo-storage, vo-frontend, vo-executor, vo-scheduler, vo-sdk, vo-sdk-macros, vo-linter, vo-common

### 9. Test File Size Violations

Test files are also exceeding limits. While less critical than production code, large test files make maintenance harder:
- vo-core replay tests: 2,121 lines
- vo-types workflow tests: 1,607 lines
- vo-types tx_coordinator tests: 1,394 lines

---

## P3 — Low Priority

### 10. TODO/FIXME Markers

7 markers across 3 crates:
- vo-api: 1
- vo-sdk: 2
- vo-storage: 3
- vo-types: 1

### 11. Architecture Spec Stale

`architecture-spec.md` references 12 crates; workspace now has 15 (vo-linter, vo-executor, vo-scheduler added later).

---

## Crate Health Scores

| Crate | Lines | Files >300 | Tests | Lint | Health |
|-------|-------|-----------|-------|------|--------|
| vo-common | 630 | 0 | 217 | YES | Excellent |
| vo-linter | 781 | 1 | 93 | YES | Good |
| vo-ipc | 2,185 | 4 | 233 | YES | Good |
| vo-executor | 3,669 | 6 | 272 | YES | Good |
| vo-scheduler | 1,492 | 1 | 66 | NO | Fair |
| vo-worker | 6,424 | 9 | 224 | YES | Fair |
| vo-frontend | 9,858 | 11 | 340 | YES | Fair |
| vo-api | 6,201 | 6 | 411 | NO | Fair |
| vo-cli | 4,888 | 4 | 1,314 | YES | Good |
| vo-sdk-macros | 788 | 1 | 61 | NO | Fair |
| vo-sdk | 6,910 | 7 | 417 | NO | Poor |
| vo-actor | 19,821 | 21 | 788 | YES | Poor |
| vo-core | 43,000 | 51 | 2,278 | NO | Poor |
| vo-storage | 36,838 | 49 | 2,070 | YES | Poor |
| vo-types | 55,422 | 61 | 3,207 | YES | Poor |

---

## Recommended Action Order

1. **Add `[lints] workspace = true` to 6 missing crates** (1 hour, high impact)
2. **Fix documentation drift** — README title, lib.rs doc comments, vision stub (2 hours)
3. **Normalize vo-cli Cargo.toml** — use workspace references (30 min)
4. **Split top-10 largest production files** — vo-actor probe.rs, lib.rs, message_router.rs; vo-storage append.rs (1-2 days)
5. **Remove warning suppressions** — vo-worker, vo-sdk, vo-sdk-macros allow directives (2 hours)
6. **Audit unwrap/expect in production code** — start with vo-storage, vo-types (2-3 days)
7. **Add per-crate moon.yml** for remaining 10 crates (4 hours)
8. **Split oversized test files** into focused test modules (1-2 days)
