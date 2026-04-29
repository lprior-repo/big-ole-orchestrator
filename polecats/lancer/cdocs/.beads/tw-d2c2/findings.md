# ARCH-DRIFT: Batch 5 — vo-linter + vo-scheduler

**Bead**: tw-d2c2  
**Analyst**: lancer  
**Date**: 2026-04-24  
**Scope**: `crates/vo-linter/`, `crates/vo-scheduler/`

---

## Summary

| Crate | Production LOC | Test LOC | Files >300 lines | Drift Severity |
|-------|---------------|----------|-------------------|----------------|
| vo-linter | ~263 | ~1309 | 1 (`rules.rs` 672L) | MEDIUM |
| vo-scheduler | ~883 | ~936 | 0 | LOW-MEDIUM |

**STATUS: FINDINGS RECORDED** (audit only, no code changes in this batch)

---

## vo-linter Findings

### F1: LINE COUNT VIOLATION — `rules.rs` at 672 lines
- **File**: `crates/vo-linter/src/rules.rs` (672 lines, limit: 300)
- **Breakdown**: ~155 lines production, ~517 lines inline `#[cfg(test)]`
- **Fix**: Extract `#[cfg(test)] mod tests` block into `tests/rules_unit_tests.rs` or `tests/inline_rules_tests.rs`. Production code would be ~155 lines — well under limit.
- **Severity**: MEDIUM

### F2: DEAD CODE — `RandomDetector` empty impl block
- **File**: `rules.rs:57` — `impl RandomDetector {}` is empty. Pure dead code.
- **Fix**: Remove the empty impl block.
- **Severity**: LOW

### F3: DEAD CODE — `Rule` trait methods `id()` and `name()` unused
- **File**: `rules.rs:97-100` — `Rule::id()` and `Rule::name()` are defined but never called by `RuleRegistry::execute_all()`. Only `execute()` is invoked.
- **Fix**: Either use these in diagnostics/logging, or remove them from the trait.
- **Severity**: LOW

### F4: DEAD CODE — `Diagnostic.code` field annotated `#[allow(dead_code)]`
- **File**: `diagnostic.rs:9` — The `code` field is never read outside tests.
- **Fix**: Either expose `code` via a public accessor or remove the field.
- **Severity**: LOW

### F5: MISSING VARIANT — `LintCode` has `L002` but no `L001`
- **File**: `diagnostic.rs:2-4` — Enum only contains `L002`. Either `L001` was removed and `L002` wasn't renumbered, or there's a missing lint rule.
- **Fix**: Add `L001` rule or renumber `L002` to `L001`.
- **Severity**: LOW

### F6: PRIMITIVE OBSESSION — `Rule::id()` and `Rule::name()` return `&'static str`
- **File**: `rules.rs:97-99` — Untyped string returns. Should be `RuleId` and `RuleName` newtypes.
- **Severity**: LOW (single implementor, low churn risk)

---

## vo-scheduler Findings

### F7: FAKE ASYNC — API functions marked `async` but never `.await`
- **File**: `api.rs:6,26,43,50` — All four public functions are `async fn` but contain zero `.await` calls. These are synchronous functions wearing async disguises.
- **Impact**: Unnecessary `Future` allocation, confusing API contract, callers forced into async context for no reason.
- **Fix**: Remove `async` from all four functions. Change return types from `impl Future<Output = Result<...>>` to direct `Result<...>`.
- **Severity**: MEDIUM

### F8: UNWRAP_OR_DEFAULT SILENTLY SWALLOWS ERRORS
- **File**: `queue.rs:123` — `chrono::Duration::from_std(*d).unwrap_or_default()` silently converts overflowed durations to zero.
- **Impact**: A `SchedulePolicy::After(584 years)` would silently become "due immediately".
- **Fix**: Return `SchedulerError::InvalidSchedule` if `from_std` fails.
- **Severity**: HIGH

### F9: PRIMITIVE OBSESSION — `RetryPolicy.backoff_multiplier` is raw `f64`
- **File**: `types.rs:177` — Backoff multiplier is `f64`. Could be `BackoffMultiplier` newtype with validation at construction.
- **Note**: `try_new` validates >= 1.0, but the field is still `pub`, allowing direct mutation.
- **Fix**: Make fields private, provide accessor methods.
- **Severity**: LOW

### F10: TYPE ALIAS NOT NEWTYPE — `SerializedPayload = bytes::Bytes`
- **File**: `job.rs:7` — `pub type SerializedPayload = bytes::Bytes` is a type alias, not a newtype. Zero type safety — interchangeable with raw `Bytes`.
- **Fix**: `pub struct SerializedPayload(bytes::Bytes);` with Deref/DerefMut.
- **Severity**: LOW

### F11: ALL PUBLIC FIELDS ON `ScheduledJob` — NO ENCAPSULATION
- **File**: `job.rs:9-23` — All fields on `ScheduledJob` are `pub`. No invariants are enforced after construction. Any caller can set `state = JobState::Running` directly, bypassing `transition()`.
- **Fix**: Make fields `pub(crate)` or private; expose through methods.
- **Severity**: MEDIUM

### F12: `last_error: Option<String>` — UNTYPED ERROR STORAGE
- **File**: `job.rs:20` — Error stored as plain `String`, losing structured error information.
- **Fix**: Use `Option<SchedulerError>` or a domain error type.
- **Severity**: LOW

### F13: STATE MACHINE NOT TYPESTATE — runtime-only validation
- **File**: `job.rs:64-85` — `ScheduledJob::transition()` validates state transitions at runtime via match. Could use typestate pattern to make invalid transitions unrepresentable at compile time.
- **Note**: This is a design improvement, not a bug. Current approach is functional but verbose.
- **Severity**: LOW (design suggestion)

### F14: `SchedulerError::SerializationError(String)` — UNTYPED
- **File**: `error.rs:14` — Wraps `String` instead of a concrete error type like `serde_json::Error`.
- **Fix**: `SerializationError(#[from] serde_json::Error)` with `#[from]`.
- **Severity**: LOW

---

## Severity Distribution

| Severity | Count |
|----------|-------|
| HIGH     | 1     |
| MEDIUM   | 3     |
| LOW      | 10    |
| **Total** | **14** |

## Priority Fix Order

1. **F8** (HIGH) — `unwrap_or_default` on duration conversion — silent data corruption
2. **F1** (MEDIUM) — `rules.rs` over 300 lines — split tests out
3. **F7** (MEDIUM) — fake async API — remove unnecessary async
4. **F11** (MEDIUM) — public fields on ScheduledJob — enforce encapsulation
5. **F2-F6, F9-F10, F12-F14** (LOW) — address in subsequent cleanup passes

## Recommended Beads to Create

- `vo-scheduler: Fix unwrap_or_default on chrono::Duration::from_std` (P0 bug)
- `vo-linter: Extract inline tests from rules.rs` (P2 task)
- `vo-scheduler: Remove fake async from api.rs` (P1 task)
- `vo-scheduler: Make ScheduledJob fields private` (P2 task)
