# Architectural Drift Analysis — twerk (Batch 5)

**Codebase:** `/home/lewis/gt/twerk/polecats/brahmin/twerk/crates/`
**Total crate source lines:** 64,755
**Date:** 2026-04-24

---

## 1. File Length Violations (>300 lines)

**Severity: HIGH**

36 non-test files exceed 300 lines. Worst offenders:

| Lines | Crate | File |
|------:|-------|------|
| 684 | twerk-infrastructure | `src/datastore/postgres/impl_jobs.rs` |
| 673 | twerk-infrastructure | `src/datastore/inmemory.rs` |
| 622 | twerk-app | `src/engine/worker/mod.rs` |
| 594 | twerk-infrastructure | `src/runtime/docker/container/factory.rs` |
| 590 | twerk-infrastructure | `src/datastore/postgres/impl_scheduled_jobs.rs` |
| 578 | twerk-infrastructure | `src/locker/postgres.rs` |
| 559 | twerk-core | `src/task.rs` |
| 531 | twerk-app | `src/engine/worker/shell.rs` |
| 515 | twerk-common | `src/conf/lookup.rs` |
| 508 | twerk-core | `src/repository_inmemory.rs` |
| 508 | twerk-core | `src/job.rs` |
| 492 | twerk-infrastructure | `src/runtime/docker/reference.rs` |
| 490 | twerk-infrastructure | `src/runtime/docker/auth/auth_config.rs` |
| 464 | twerk-core | `src/user.rs` |
| 447 | twerk-web | `src/openapi.rs` |
| 418 | twerk-app | `src/engine/datastore/proxy.rs` |
| 412 | twerk-infrastructure | `src/datastore/postgres/impl_tasks.rs` |
| 408 | twerk-common | `src/syncx/map.rs` |
| 375 | twerk-common | `src/reexec.rs` |
| 363 | twerk-infrastructure | `src/runtime/docker/container/tcontainer.rs` |
| 363 | twerk-cli | `src/commands.rs` |
| 356 | twerk-infrastructure | `src/runtime/docker/reference_test.rs` |
| 353 | twerk-infrastructure | `src/broker/mod.rs` |
| 352 | twerk-core | `src/redact.rs` |
| 339 | twerk-app | `src/engine/worker/podman.rs` |
| 331 | twerk-core | `src/asl/types.rs` |
| 331 | twerk-app | `src/engine/coordinator/handlers/job_handlers.rs` |
| 330 | twerk-infrastructure | `src/runtime/docker/archive.rs` |
| 329 | twerk-app | `src/engine/coordinator/handlers/task_handlers.rs` |
| 326 | twerk-cli | `src/handlers/trigger.rs` |
| 323 | twerk-core | `src/eval/intrinsics.rs` |
| 313 | twerk-infrastructure | `src/cache/tests.rs` |
| 310 | twerk-web | `src/api/openapi.rs` |
| 303 | twerk-infrastructure | `src/runtime/docker/auth/auth_resolver.rs` |
| 303 | twerk-app | `src/engine/broker.rs` |

**Recommendation:** Split largest files using Extract Module/Extract Function. Priority: `impl_jobs.rs` (split by query type), `datastore/inmemory.rs` (separate into entity modules), `worker/mod.rs` (extract trait impls), `task.rs` (separate parsing from domain logic).

---

## 2. Circular Dependencies

**Severity: NONE**

The inter-crate dependency chain forms a clean DAG:

```
twerk-common (leaf)
    <- twerk-core
        <- twerk-infrastructure
            <- twerk-app
                <- twerk-web
                    <- twerk-cli
```

No cycles detected.

---

## 3. Architectural Boundary Violations

**Severity: HIGH**

`twerk-web` depends on `twerk-app` — an **upward dependency violation**. The HTTP API layer should not import engine internals.

**Recommendation:** Extract interfaces that `twerk-web` needs from `twerk-app` into `twerk-core` traits. Remove `twerk-app` dependency from `twerk-web`.

---

## 4. Public API Surface Drift

**Severity: MEDIUM**

| Crate | pub fn | pub struct | pub enum | pub trait |
|-------|--------|-----------|---------|----------|
| twerk-core | 264 | 81 | 56 | 3 |
| twerk-infrastructure | 131 | 65 | 23 | 16 |
| twerk-web | 105 | 45 | 19 | 0 |
| twerk-app | 97 | 37 | 14 | 2 |
| twerk-common | 78 | 5 | 4 | 0 |
| twerk-cli | 4 | 12 | 11 | 0 |

`twerk-core` has accumulated significant domain logic (ASL evaluation, trigger registries, validation, redaction, cron parsing, webhook handling). Consider extracting into a `twerk-domain` crate.

---

## 5. Dead Code Indicators

**Severity: MEDIUM**

22 `#[allow(dead_code)]` annotations in production code:

- **twerk-app (6):** Worker implementations have dead struct fields — suggests incomplete worker trait refactor
- **twerk-cli (3):** Unfinished command implementations in dispatch/help modules
- **twerk-core (8):** `domain/testing.rs` has 7 dead-code suppressions — gate behind `#[cfg(test)]` or feature flag
- **twerk-common (3):** Dead struct fields in config types
- **twerk-infrastructure (2):** Dead fields in docker runtime (one annotated "used in integration tests")

**Recommendation:** Remove dead fields or complete trait refactor. Gate testing module.

---

## 6. Error Handling Consistency

**Severity: NONE (HEALTHY)**

Consistent `thiserror` + `anyhow` pattern across all crates. `twerk-core` has 37 well-scoped error types. No action needed.

---

## 7. Test Coverage Gaps

**Severity: HIGH**

| Crate | Total src files | With `#[cfg(test)]` | Without tests | Coverage % |
|-------|----------------|--------------------:|--------------:|-----------:|
| twerk-app | 50 | 12 | 38 | 24% |
| twerk-cli | 18 | 9 | 9 | 50% |
| twerk-common | 15 | 8 | 7 | 53% |
| twerk-core | 86 | 27 | 59 | 31% |
| twerk-infrastructure | 104 | 25 | 79 | 24% |
| twerk-web | 54 | 9 | 45 | 17% |

Largest untested files (>500 lines, no `#[cfg(test)]`):
- `impl_jobs.rs` (684 lines)
- `inmemory.rs` (673 lines)
- `worker/mod.rs` (622 lines)
- `container/factory.rs` (594 lines)
- `impl_scheduled_jobs.rs` (590 lines)
- `task.rs` (559 lines)
- `lookup.rs` (515 lines)

**Note:** Project has 120+ integration test files (adversarial, BDD, red-queen, proptest, benchmark). Unit test gaps partially compensated.

---

## 8. Dependency Health

**Severity: LOW**

- One non-workspace version: `indexmap` in `twerk-core` — move to workspace
- Dual `sqlx`/`postgres` native drivers in `twerk-infrastructure` — investigate if both needed
- Dual `ureq`/`reqwest` HTTP clients — acceptable (sync vs async), document rationale

---

## Summary by Severity

| Severity | Count | Key Issues |
|----------|------:|------------|
| CRITICAL | 0 | — |
| HIGH | 3 | File length (36 files), boundary violation (web->app), test gaps (17-24% unit coverage) |
| MEDIUM | 3 | Core API bloat (264 pub fn), dead code (22 annotations), testing module gating |
| LOW | 2 | Error handling healthy, deps mostly clean |
