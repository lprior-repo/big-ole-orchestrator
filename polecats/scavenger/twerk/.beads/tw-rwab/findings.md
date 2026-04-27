# GO-PLAN: twerk module plan 28 — Findings

## Scope: Type Safety Hardening in twerk-core

### Problem Statement

The `twerk-core` crate defines validated newtype wrappers (IDs, Port, Progress, etc.) intended to make illegal states unrepresentable. However, every single validated type provides **infallible bypass constructors** (`From<T>`, `Default`) that silently skip validation. This defeats the entire purpose of these types — invalid data can be constructed anywhere in the codebase without compiler resistance.

### Affected Types

#### 1. ID Types (`crates/twerk-core/src/id/`)

**All 6 ID types derive `Default` and implement infallible `From<String>` + `From<&str>`:**

| Type | File | Default gives | From<String> validates? |
|------|------|---------------|------------------------|
| `TaskId` | `common.rs:108` (macro) | `""` (empty string) | NO — skips `validate_id()` |
| `NodeId` | `common.rs:109` (macro) | `""` (empty string) | NO |
| `ScheduledJobId` | `common.rs:110` (macro) | `""` (empty string) | NO |
| `UserId` | `common.rs:111` (macro) | `""` (empty string) | NO |
| `RoleId` | `common.rs:112` (macro) | `""` (empty string) | NO |
| `JobId` | `job_id.rs:44` | `""` (empty string) | NO — skips `validate_job_id()` |
| `TriggerId` | `trigger_id.rs:10` | `""` (empty string) | NO — skips TriggerId-specific validation |

**Root cause**: The `define_id!` macro on `common.rs:41-106` generates `Default` and `From<String>` for all 5 generic ID types. `JobId` and `TriggerId` are hand-written but replicate the same pattern.

**Callsites using bypasses (non-test)**:
- `NodeId::from(id)` in `app/engine/worker/mod.rs:255` — node ID from runtime without validation
- `NodeId::from(id.clone())` in `infrastructure/worker/internal/heartbeat.rs:63` — same pattern

**Callsites using bypasses (test-only)**:
- `RoleId` test at `core/role.rs:205`
- `UserId` tests at `core/user.rs:382,388`

#### 2. Primitive Wrapper Types (`crates/twerk-core/src/types/`)

| Type | File | Has `Default`? | Has infallible `From<T>`? | Validates in Deserialize? |
|------|------|----------------|--------------------------|--------------------------|
| `Port` | `port.rs:15` | NO | YES `From<u16>` at line 71 | YES |
| `Progress` | `progress.rs:14` | NO | YES `From<f64>` at line 75 | YES |
| `RetryLimit` | `retry_limit.rs:12` | NO | NO (has `TryFrom<u32>`) | YES |
| `TaskCount` | `task_count.rs:14` | NO | YES `From<u32>` at line 69 | NO (uses `#[derive(Deserialize)]`) |
| `TaskPosition` | `task_position.rs:15` | NO | YES `From<i64>` at line 62 | NO (uses `#[derive(Deserialize)]`) |
| `RetryAttempt` | `retry_attempt.rs:14` | NO | YES `From<u32>` at line 61 | NO (uses `#[derive(Deserialize)]`) |

**Key finding**: `TaskCount`, `TaskPosition`, and `RetryAttempt` have infallible `From` AND skip validation in `Deserialize` (they use `#[derive(Deserialize)]` instead of a manual impl). This means invalid values can enter via both code AND JSON.

### Impact Assessment

| Severity | Impact | Example |
|----------|--------|---------|
| **HIGH** | Empty ID strings in DB/state machine | `JobId::default()` creates `""` which fails all validation rules |
| **HIGH** | Invalid chars in IDs bypassing validation | `From<String>` lets `"@#$%"` become a `TaskId` |
| **MEDIUM** | Port 0 bypasses range check | `Port::from(0u16)` creates invalid port |
| **MEDIUM** | Progress NaN/out-of-range | `Progress::from(f64::NAN)` or `Progress::from(999.0)` |
| **LOW** | TaskCount/TaskPosition always succeed | These are effectively just newtype wrappers with no invariant |

### Proposed Fix Strategy

#### Phase 1: Remove infallible constructors from ID types
1. Remove `Default` from all 7 ID types (breaks any code relying on empty IDs)
2. Remove `From<String>` and `From<&str>` from all 7 ID types
3. Update `define_id!` macro to stop generating these
4. Fix `NodeId::from()` callsites in `worker/mod.rs` and `heartbeat.rs` to use `NodeId::new()`
5. Update tests to use `TaskId::new("valid-id").unwrap()` or `TaskId::new_unchecked()` for test fixtures

#### Phase 2: Remove infallible constructors from primitive wrappers
1. `Port`: Change `From<u16>` to `TryFrom<u16>` (already has proper `FromStr`)
2. `Progress`: Change `From<f64>` to `TryFrom<f64>`
3. `TaskCount`: Add manual `Deserialize` impl with validation; change `From<u32>` to `TryFrom`
4. `TaskPosition`: Add manual `Deserialize` impl; change `From<i64>` to `TryFrom`
5. `RetryAttempt`: Change `From<u32>` to `TryFrom`

#### Phase 3: Add test coverage
1. Property tests for all ID validation (via `proptest`)
2. Property tests for Port, Progress, RetryLimit ranges
3. Verify Deserialize rejects invalid values
4. Verify no code path can construct invalid types

#### Phase 4: Add clippy lint
1. Add `#![deny(clippy::unwrap_used)]` to twerk-core
2. Add `#![deny(clippy::expect_used)]` where possible

### Files to Change

| File | Change |
|------|--------|
| `crates/twerk-core/src/id/common.rs` | Remove `Default` and `From` from `define_id!` macro |
| `crates/twerk-core/src/id/job_id.rs` | Remove `Default`; add `From<JobId> for String` only |
| `crates/twerk-core/src/id/trigger_id.rs` | Remove `Default`; remove `From<String>` and `From<&str>` |
| `crates/twerk-core/src/types/port.rs` | Change `From<u16>` to `TryFrom<u16>` |
| `crates/twerk-core/src/types/progress.rs` | Change `From<f64>` to `TryFrom<f64>` |
| `crates/twerk-core/src/types/task_count.rs` | Change `From<u32>` to `TryFrom<u32>`; add manual Deserialize |
| `crates/twerk-core/src/types/task_position.rs` | Change `From<i64>` to `TryFrom<i64>`; add manual Deserialize |
| `crates/twerk-core/src/types/retry_attempt.rs` | Change `From<u32>` to `TryFrom<u32>` |
| `crates/twerk-app/src/engine/worker/mod.rs` | Fix `NodeId::from(id)` → `NodeId::new(id)?` |
| `crates/twerk-infrastructure/src/worker/internal/heartbeat.rs` | Fix `NodeId::from(id.clone())` → `NodeId::new(id.clone())?` |
| `crates/twerk-core/src/role.rs` | Fix test: use `RoleId::new("from-str").unwrap()` |
| `crates/twerk-core/src/user.rs` | Fix tests: use `UserId::new("...").unwrap()` |
| All files using `Port::from()` | Change to `Port::new()?.into()` or `Port::try_from()` |
| All files using `Progress::from()` | Change to `Progress::try_from()` |

### Blast Radius Analysis

Removing `From<String>` and `Default` from ID types is a **breaking API change** within the workspace. Every callsite that relies on infallible construction will fail to compile. This is intentional — the compiler will identify every location where unvalidated IDs are created.

**Estimated callsites to fix**: 2 non-test production callsites (both `NodeId::from`), ~3 test callsites, plus any indirect uses through trait resolution.

### Risk Assessment

| Risk | Mitigation |
|------|-----------|
| Breaks downstream crate compiles | Expected — fix callsites in same PR |
| Tests break | Expected — update to use `::new().unwrap()` for fixtures |
| Serde round-trip breaks | JobId and TriggerId already have custom Deserialize; macro-generated IDs need manual Deserialize impls |
| Default required by some trait bound | Remove the bound or provide a `#[cfg(test)]` fixture constructor |

### Dependencies

- None — this is a self-contained refactoring within twerk-core with minor fixes in twerk-app and twerk-infrastructure
