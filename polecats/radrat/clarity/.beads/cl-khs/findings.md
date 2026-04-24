# Architectural Drift Detection Report — wave3-15

**Rig**: clarity  
**Bead**: cl-khs  
**Date**: 2026-04-24  
**Analyzer**: polecat radrat  
**Scope**: `/home/lewis/gt/clarity/polecats/brahmin/clarity/clarity-web/`

---

## Executive Summary

**STATUS: SEVERE DRIFT**

The clarity codebase is in a state of severe architectural drift. The codebase has 329 Rust files totaling 251,737 lines (average 765 lines/file). **139 files (42%) exceed the 300-line limit.** The top 10 worst offenders range from 1509–2778 lines (5x–9x the threshold).

Three systemic violations dominate:
1. **Primitive obsession** — zero NewType wrappers for IDs, names, timestamps, scores, or priorities across the entire codebase
2. **Validate, don't parse** — no validation function produces a typed guarantee; all return `Result<(), Error>` or `bool`
3. **File bloat** — 139 files exceed 300 lines; tests are co-located inflating size by 30–63%

---

## Quantitative Overview

| Metric | Value |
|--------|-------|
| Total `.rs` files | 329 |
| Total lines | 251,737 |
| Average file size | 765 lines |
| Files >300 lines | **139 (42%)** |
| Files >1000 lines | **48** |
| Files >1500 lines | **14** |
| `.unwrap()` calls in src/ | 316 |
| `.clone()` calls in src/ | 705 |
| `.expect()` calls in src/ | 500 |

---

## CRITICAL Findings

### C1: `server.rs` is a 2778-line God File

The single worst file in the codebase at **9.3x** the 300-line limit. Contains:
- Rate limiting (`RateLimiter`)
- AI provider bootstrapping (`AiProviderState`)
- Schema building (multiple schema builders)
- EARS extraction
- KIRK compilation
- Field extraction proxying
- Quality scoring proxying
- Straw man validation proxying
- Hole punching proxying
- Antithesis/VORP validation
- 37 unit tests (879 lines = 32% of file)
- Type definitions that duplicate types elsewhere

### C2: Domain Layer is Dead Code

`src/domain/` defines `Answer`, `Spec`, `Feature`, `Behavior`, `AnswerId`, `StepId`, `BeadId`, `QualityEvaluator<T>` — but **zero consumers outside `domain/`**. The rest of the codebase imports `Answer` from `types.rs`, `intent/interview/types.rs`, or `server.rs`. The domain newtypes have zero usage.

### C3: Massive Type Duplication

| Type | Location 1 | Location 2 | Location 3 |
|------|-----------|-----------|-----------|
| `Answer` | `types.rs` | `domain/types.rs` | `intent/interview/types.rs` |
| `Phase` | `types.rs` | `server.rs` (enum) | `lattice/compact.rs` (enum) |
| `PlanError`, `PhaseStatus`, `BeadStatus`, `PlanBead`, `Phase`, `ExecutionPlan` | `intent/plan/plan_mode.rs` | `intent/plan/plan_mode/types.rs` (exact duplicates) | — |

### C4: Inverted Dependency — Storage Imports from UI

`storage/mod.rs:34`:
```rust
pub use crate::components::discover::types::{HolePunchingResults, ScenarioField};
```
The persistence layer depends on UI component types. This violates hexagonal architecture.

### C5: Zero NewType IDs Across Entire Codebase

Every identifier is a raw `String`: `bead_id`, `session_id`, `question_id`, `requirement_a`, `node_id`, `story_id`, `character_id`, `entry_id`, `contract_id`. No compile-time protection against mixing ID types.

### C6: Validate, Don't Parse — Systemic

- `validate_email(&str) -> Result<(), FormatError>` — produces no type
- `validate_uuid(&str) -> Result<(), UuidError>` — produces no type
- `validate_uri(&str) -> Result<(), UriError>` — produces no type
- `apply_rule(&rule, value: &str) -> Result<RuleResult, RuleError>` — validated value stays as `&str`
- `Character::is_valid(&self) -> bool` — validate-after-construction pattern
- `DiscoveryMechanism::is_valid(&self) -> bool` — same

---

## HIGH Findings

### H1: Top 14 Files Exceeding 1500 Lines

| File | Lines | Severity |
|------|-------|----------|
| `server.rs` | 2778 | CRITICAL |
| `intent/interview/answer_extraction.rs` | 2559 | CRITICAL |
| `intent/quality/effects.rs` | 1903 | HIGH |
| `intent/quality/improver.rs` | 1761 | HIGH |
| `intent/beads/templates.rs` | 1701 | HIGH |
| `lattice/quality.rs` | 1560 | HIGH |
| `intent/quality/linter.rs` | 1540 | HIGH |
| `intent/validation/semantic.rs` | 1535 | HIGH |
| `kirk/terminal_integration.rs` | 1528 | HIGH |
| `hooks/progressive_discover.rs` | 1509 | HIGH |

### H2: Pervasive Mutable Accumulation Pattern

Multiple files use `&mut Vec<Issue>` parameters or `&mut self` methods to accumulate findings:
- `lattice/quality.rs`: 37 `let mut`, 46 `&mut` (highest count)
- `hooks/progressive_discover.rs`: 9 `let mut`, 20 `&mut`
- `pme/infra/testing.rs`: entire `CoverageTracker` is mutation-based
- `pme/infra/tracing.rs`: `Span` state transitions via `&mut self`
- `conflict_detection.rs`: `conflict_id: &mut usize` threaded through 6+ functions

Functions should return collections instead of mutating accumulators.

### H3: Silent Error Swallowing

- `pme/infra/metrics.rs`: `if let Ok(mut sum) = self.state.sum.lock()` — silently discards lock poisoning
- `pme/infra/tracing.rs`: `Tracer::record_span` silently swallows lock failures; `get_spans` returns empty Vec on failure
- `intent/quality/analyzer.rs`: `score.min(100)` clamping silently hides overflow bugs

### H4: Tests Co-located Inflating File Sizes

| File | Test % | Production Lines |
|------|--------|-----------------|
| `intent/quality/improver.rs` | 61% | ~695 |
| `lattice/quality.rs` | 63% | ~581 |
| `intent/interview/answer_extraction.rs` | 50% | ~1270 |
| `intent/quality/effects.rs` | 42% | ~1109 |
| `intent/beads/templates.rs` | 43% | ~965 |
| `pme/infra/testing.rs` | 35% | ~830 |
| `pme/infra/tracing.rs` | 33% | ~810 |
| `kirk/terminal_integration.rs` | 36% | ~972 |

### H5: `hooks/progressive_discover.rs` Mixes 5 Architectural Layers

Single file contains: localStorage persistence, crash recovery, state machine transitions, Dioxus reactive signals, and business logic validation. No validation on recovered state from localStorage.

### H6: `intent/` is a Self-Contained Monolith

The `intent/` module is a full Gleam CLI port with its own types, interview system, quality analysis, validation, beads generation, planning, and document handling. It has 21+ subdirectories and its own duplicate type system.

### H7: `pme/infra/` Contains Complete Libraries in Single Files

- `metrics.rs` (1330 lines): Counter, Gauge, Histogram, Registry, RUM Collector
- `testing.rs` (1282 lines): TestResult, TestSummary, TestFixture, CoverageTracker, TestDataGenerator
- `tracing.rs` (1213 lines): TraceId, SpanId, Span, SpanBuilder, Tracer, TraceContext

---

## MEDIUM Findings

### M1: Bounded Number Types Without Type Enforcement

- `priority: u8` (range 1-5 or 1-10) — runtime validation only
- `score: u8` (range 0-100) — `u8` allows 0-255
- `timeout_ms: u64` — any value valid including 0
- `retry_delay_ms: u64` — same
- `probability: f64` — should be `0.0..=1.0`

### M2: `Default` Implementations Return Invalid States

`BeadTemplate::default()` returns empty/invalid template. `TestFixture` uses `Option::take()` for one-time execution. These violate "make illegal states unrepresentable."

### M3: Stringly-Typed Custom Rules

`Rule::Custom { name: String, check: String }` — the `check` field is a mini-DSL parsed at runtime with string matching. Should be a typed closure or proper AST.

### M4: 34 Clippy Suppressions in `plan_mode/types.rs`

`#[allow(...)]` suppresses: `too_many_lines`, `needless_pass_by_value`, `collection_is_never_read`, `needless_collect`. These are genuine code quality issues being hidden.

### M5: Boolean Trap in `ValidationResult`

`ValidationResult` has `is_valid: bool` alongside `errors: Vec<...>`. The boolean is always derivable as `errors.is_empty()` and can drift out of sync.

---

## Positive Patterns (What's Working)

- **Strict clippy lints**: `#![warn(clippy::pedantic)]`, `#![warn(clippy::nursery)]`, `#![forbid(unsafe_code)]`
- **`thiserror`** consistently used for error derivation
- **Builder pattern** (`with_*` methods) consistently applied
- **`#[must_use]`** properly applied to pure functions
- **State machine transitions** properly encoded with exhaustive pattern matching in `PhaseStatus` and `BeadStatus`
- **No compile-time circular dependencies** (Rust enforces this)
- **Dependencies are lean**: 15 runtime deps, properly gated with conditional compilation
- **`SemanticError`** is a model error type with 8 structured variants
- **`AnswerFileError`** has excellent error classification with location tracking
- **`TerminalClient` trait** with `async_trait` is a proper hexagonal boundary
- **`EarsRequirement`** tagged enum makes illegal states unrepresentable

---

## Recommended Remediation Priority

### Phase 1 (Immediate — Reduce file bloat)
1. Extract all inline `#[cfg(test)]` modules into separate test files
2. Split `server.rs` into `server/` directory with 8+ modules
3. Split `pme/infra/` libraries into individual files per type

### Phase 2 (Short-term — Type safety)
1. Introduce NewType wrappers for all IDs: `BeadId`, `SessionId`, `QuestionId`, etc.
2. Replace `validate_* -> Result<(), Error>` with `parse_* -> Result<ValidatedType, Error>`
3. Consolidate duplicate types — single source of truth in `domain/`
4. Introduce `Percentage`, `Priority`, `NonEmptyString` smart types

### Phase 3 (Medium-term — Architecture)
1. Make `domain/` the canonical type source (or remove it)
2. Move types from `components/discover/types.rs` to `domain/`
3. Fix inverted dependency: `storage/` must not import from `components/`
4. Extract `intent/` into a separate crate or deeply refactor into sub-modules

### Phase 4 (Long-term — Patterns)
1. Replace mutable accumulation with functional fold/collect
2. Replace `&mut Vec<Issue>` with return-based collection
3. Introduce proper error handling for lock poisoning (no silent swallow)
4. Replace stringly-typed DSL in `Rule::Custom` with typed AST
