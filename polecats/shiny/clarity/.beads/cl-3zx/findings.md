# Architectural Drift Detection — wave3-1

**Date:** 2026-04-24
**Rig:** clarity
**Project:** clarity-web (Dioxus 0.7 fullstack app)
**Total .rs files in src/:** ~200+
**Top-level modules:** 14 (app, bin, components, config, domain, hooks, intent, kirk, lattice, pages, pme, providers, storage, ui)

---

## 1. FILE SIZE VIOLATIONS (>300 lines)

These files exceed the <300 line limit and require decomposition:

| File | Lines | Severity |
|------|-------|----------|
| `src/server.rs` | **2778** | CRITICAL — god file, ~10x limit |
| `src/intent/interview/answer_extraction.rs` | **2559** | CRITICAL — ~8.5x limit |
| `src/intent/quality/effects.rs` | **1903** | CRITICAL — ~6x limit |
| `src/intent/quality/improver.rs` | **1761** | CRITICAL — ~6x limit |
| `src/intent/beads/templates.rs` | **1701** | CRITICAL — ~5.7x limit |
| `src/intent/validation/semantic.rs` | **1535** | CRITICAL — ~5x limit |
| `src/hooks/progressive_discover.rs` | **1509** | CRITICAL — ~5x limit |
| `src/intent/interview/answer_file.rs` | **1418** | CRITICAL — ~4.7x limit |
| `src/components/discover/progressive_discover.rs` | **1370** | CRITICAL — ~4.6x limit |
| `src/intent/quality/analyzer.rs` | **1297** | CRITICAL — ~4.3x limit |
| `src/intent/validation/spec_validator.rs` | **1278** | CRITICAL — ~4.3x limit |
| `src/lattice/conflict_detection.rs` | **1159** | CRITICAL — ~3.9x limit |
| `src/lattice/quality_dimensions.rs` | **1139** | CRITICAL — ~3.8x limit |
| `src/kirk/progressive_discover.rs` | **998** | HIGH — ~3.3x limit |
| `src/lattice/interview_5x5.rs` | **974** | HIGH — ~3.2x limit |
| `src/lattice/inversion.rs` | **947** | HIGH — ~3.2x limit |
| `src/lattice/design_by_contract.rs` | **911** | HIGH — ~3x limit |
| `src/providers/opencode.rs` | **891** | HIGH — ~3x limit |
| `src/intent/types/names.rs` | **887** | HIGH — ~3x limit |
| `src/lattice/coverage.rs` | **884** | HIGH — ~3x limit |
| `src/intent/plan/plan_next.rs` | **833** | HIGH — ~2.8x limit |
| `src/lattice/ears.rs` | **776** | HIGH — ~2.6x limit |
| `src/intent/cli/validation.rs` | **663** | HIGH — ~2.2x limit |
| `src/intent/plan/types.rs` | **660** | HIGH — ~2.2x limit |
| `src/components/graph_visualizer.rs` | **607** | HIGH — ~2x limit |

**Summary:** 25 files exceed 300 lines. 11 files exceed 1000 lines. 3 files exceed 2000 lines.
**Total violation surface:** ~27,000 lines in oversized files.

---

## 2. LINT VIOLATIONS (workspace lints are deny/warn)

### 2a. `.unwrap()` usage — 316 occurrences across 38 files

Workspace has `unwrap_used = "deny"`. Top offenders:

| File | Count |
|------|-------|
| `intent/validation/semantic_bdd_tests.rs` | 28 |
| `intent/interview/answer_file.rs` | 27 |
| `intent/interview/types/tests/adversarial.rs` | 15 |
| `intent/cli/validation.rs` | 20 |
| `lattice/interview_5x5.rs` | 20 |
| `lattice/design_by_contract.rs` | 16 |
| `providers/trait.rs` | 20 |
| `intent/quality/effects.rs` | (counted in 86 total) |
| `server.rs` | 24 |

Note: Some may be in `#[cfg(test)]` blocks where workspace allows relaxations, but 316 total is extreme.

### 2b. `.expect()` usage — 497 occurrences across 32 files

Workspace has `expect_used = "deny"`. Top offenders:

| File | Count |
|------|-------|
| `intent/quality/effects.rs` | 83 |
| `intent/plan/types.rs` | 43 |
| `intent/plan/plan_next.rs` | 32 |
| `intent/beads/templates.rs` | 25 |
| `intent/validation/spec_validator.rs` | 33 |
| `storage/path_util.rs` | 22 |
| `intent/quality/analyzer.rs` | 30 |
| `intent/interview/answer_file.rs` | (counted in 497 total) |

### 2c. `panic!`/`todo!`/`unimplemented!` — 86 occurrences across 18 files

All three are workspace-deny. Top offenders: `server.rs` (24), `intent/quality/linter.rs` (6), `intent/formats.rs` (5), `intent/interview/types/tests/*.rs`.

### 2d. `unsafe` — 0 occurrences

PASS. `unsafe_code = "forbid"` is respected.

---

## 3. ARCHITECTURAL DRIFT CONCERNS

### 3a. God File: `server.rs` (2778 lines)

This single file mixes:
- AI provider initialization and singleton management
- Rate limiting logic
- Field extraction server functions
- Quality scoring server functions
- Straw man validation
- Multiple `#[server]` function definitions
- Provider configuration resolution

This should be decomposed into: `server/providers.rs`, `server/extraction.rs`, `server/quality.rs`, `server/rate_limit.rs`, etc.

### 3b. Name Collision: 3x `progressive_discover.rs`

Three files share the same name across different modules:
- `src/hooks/progressive_discover.rs` (1509 lines)
- `src/components/discover/progressive_discover.rs` (1370 lines)
- `src/kirk/progressive_discover.rs` (998 lines)

This suggests unclear separation of concerns or copy-paste drift between layers.

### 3c. Module Imbalance: `intent/` subtree

The `intent/` module contains ~60%+ of all oversized files. Sub-modules with high file counts:
- `intent/interview/` — 15+ files
- `intent/validation/` — 10+ files
- `intent/quality/` — 8+ files
- `intent/plan/` — 10+ files
- `intent/beads/` — 8+ files

The `intent/` module is growing into a monolith. Consider splitting into workspace crates or at least re-module-izing.

### 3d. Test Coverage

145 files contain `#[cfg(test)]` blocks — good coverage breadth. However, the largest files (server.rs at 2778 lines, answer_extraction.rs at 2559 lines) likely need their own dedicated test files rather than inline tests.

---

## 4. ROOT-LEVEL JUNK FILES (not in src/)

**Stray .rs files in repo root (not in any crate):**
- `express_flow.rs` (24K) — orphan, not part of any crate
- `test_ars_minimal.rs` (4K) — orphan test
- `test_hole_punching.rs` (12K) — orphan test
- `test_path_util.rs` (8K) — orphan test
- `test_quality_score.rs` (4K) — orphan test
- `test_quality_standalone.rs` (891B) — orphan test

**Stray scripts:**
- `fix_answer.py`, `fix_parser.py`, `fix_remaining.py` — one-off fix scripts

**Stray data files:**
- `batch_tasks_1.json` through `batch_tasks_15.json` (15 files, ~8K each)
- `foundation_tasks.json`, `oya-yoz3-task.json`, `requirements.json`, `task-001.json`, `task-002.json`
- `clippy-output.txt` (96K) — lint output artifact
- `import.sql` (616K) — SQL dump

**Build artifacts not gitignored:**
- `node_modules/` (17M)
- `mutants.out/` (1.7M), `mutants.out.old/` (344K)
- `playwright-report/` (520K)
- `test-results/` (12K)
- `rounds/` (96K)
- `refactor code/` (576K) — directory with space in name
- `test_hole_punching` (11K binary)
- `stdout.txt`, `stderr.txt` — runtime artifacts

---

## 5. SUMMARY SCORE

| Category | Finding | Severity |
|----------|---------|----------|
| File sizes | 25 files >300 lines, 3 >2000 lines | CRITICAL |
| unwrap() | 316 occurrences across 38 files | HIGH |
| expect() | 497 occurrences across 32 files | HIGH |
| panic/todo/unimpl | 86 occurrences across 18 files | HIGH |
| unsafe | 0 | PASS |
| God file | server.rs at 2778 lines | CRITICAL |
| Name collisions | 3x progressive_discover.rs | MEDIUM |
| Module imbalance | intent/ subtree is a monolith | MEDIUM |
| Root junk | ~20 stray files, ~20MB artifacts | MEDIUM |
| Test coverage | 145 files with tests (good breadth) | OK |

**Overall drift assessment: HIGH** — The codebase has significant architectural drift, primarily from oversized files and lint violations. The server.rs god file and intent/ subtree monolith are the highest-priority targets for refactoring.
