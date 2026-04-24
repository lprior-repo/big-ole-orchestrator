# GO-PLAN: twerk Module Plan 12

## Bead: tw-3dur
## Date: 2026-04-24
## Status: Analysis Complete

---

## Executive Summary

Twerk is a Rust-based workflow orchestration engine with 7 crates totaling ~150+ Rust source files. The codebase has a comprehensive `improvements.md` with 21 prioritized remediation items covering correctness, type safety, observability, and documentation. This plan synthesizes the current state and proposes an actionable implementation roadmap for the next cycle of work.

---

## Architecture Overview

```
twerk/
├── crates/
│   ├── twerk-common       # Shared utilities: config, env, logging, process control
│   ├── twerk-core          # Domain types: IDs, jobs, tasks, triggers, validation, repos
│   ├── twerk-infrastructure  # External integrations: Postgres, RabbitMQ, Docker, Podman
│   ├── twerk-app           # Engine: coordinator, scheduler, worker, broker, lifecycle
│   ├── twerk-web           # HTTP API: axum handlers, OpenAPI, error taxonomy
│   ├── twerk-cli           # CLI: clap-based, handlers for jobs/tasks/queues/triggers
│   └── twerk-openapi-gen   # OpenAPI spec generator
├── plans/                  # Existing plans (ai-ergonomics, flesh-out-ai-ergonomics)
├── qa/                     # QA test YAML files (some stale: $TORK_OUTPUT → $TWERK_OUTPUT)
├── docs/                   # Documentation
├── examples/               # Example workflow YAMLs
└── tests/                  # Various test suites (some stale)
```

### Dependency Flow
```
twerk-common (base)
    ↑
twerk-core (domain)
    ↑
twerk-infrastructure (adapters)
    ↑
twerk-app (engine) ← twerk-web (HTTP) ← twerk-cli (CLI)
```

---

## Current State Assessment

### Strengths
- Clean crate separation with clear dependency boundaries
- Comprehensive test infrastructure (BDD, contract, adversarial, chaos engineering benchmarks)
- Strong domain modeling in twerk-core (validated IDs, typed wrappers)
- Existing improvements.md provides detailed remediation roadmap

### Critical Issues (from improvements.md Priority 0)

1. **Cancellation reports success** - Cancelled tasks can be marked Completed instead of Cancelled
2. **Timeout not authoritative** - Malformed timeouts silently disable enforcement
3. **Shutdown lifecycle incomplete** - Signal handlers don't complete termination state

### High-Priority Issues (Priority 1-2)

4. Validated IDs bypassable via `Default` and infallible `From` impls
5. `Endpoint::new_unchecked()` is public, `as_url()` can panic
6. Primitive wrappers (Port, Progress, RetryLimit) allow invalid construction
7. API parsing too tolerant - silently defaults bad input
8. Persistence hides corruption via `unwrap_or_default()`
9. Task persistence contracts drift from runtime contracts
10. Config/duration parsing inconsistent across modules

### Observability & UX Issues (Priority 3-4)

11. Events publish stale pre-update payloads
12. Scheduler/cancellation semantics inconsistent
13. In-memory repository too stringly-typed and permissive
14. Logging setup can panic (uses `.init()` not `try_init()`)
15. CLI JSON mode not a real contract (hand-built strings)
16. API error taxonomy too coarse

### Documentation & Hygiene (Priority 5)

17. Generated website content stale
18. QA assets contain broken details ($TORK_OUTPUT)
19. Some tests stale enough to mislead
20. Tracked cache artifacts need repo policy
21. No workspace-wide lint policy

---

## Proposed Implementation Plan (Batch 12)

### Batch 12 Focus: Priority 0 Correctness Fixes + ID Hardening

This batch targets the highest-impact correctness issues that make the system "say one thing while doing another."

#### Phase 1: Cancellation Truthfulness (Priority 0, Item 1)

**Files:**
- `crates/twerk-infrastructure/src/worker/internal/execution.rs`
- `crates/twerk-infrastructure/src/worker/internal/worker.rs`
- `crates/twerk-app/src/engine/worker/mod.rs`

**Changes:**
- Introduce `TaskState::Cancelled` as distinct result path
- Make worker shutdown enumerate and explicitly stop active tasks
- Wire subscription loops to select on cancellation tokens
- Remove success return values from cancellation paths

**Acceptance:** Cancelling a task → persisted state is `Cancelled`, never `Completed`

#### Phase 2: Timeout Authority (Priority 0, Item 2)

**Files:**
- `crates/twerk-infrastructure/src/worker/internal/execution.rs`
- `crates/twerk-infrastructure/src/runtime/docker/runtime/mod.rs`
- `crates/twerk-app/src/engine/worker/podman.rs`
- `crates/twerk-app/src/engine/worker/shell.rs`

**Changes:**
- Parse timeout once at boundary into validated `Duration` type
- Reject malformed timeout values (fail task definition, not runtime)
- Make Docker/Podman stop paths real and testable
- Replace shell `wait` with child-handle-based waiting

**Acceptance:** Malformed timeout fails at definition; timed-out tasks are actually stopped

#### Phase 3: Shutdown Lifecycle (Priority 0, Item 3)

**Files:**
- `crates/twerk-app/src/engine/engine_lifecycle.rs`
- `crates/twerk-app/src/engine/worker/mod.rs`
- `crates/twerk-cli/src/run.rs`

**Changes:**
- Centralize termination in one explicit engine lifecycle path
- Signal handlers call same termination function as normal shutdown
- Reverse startup order for cleanup-on-failure
- Add timeout to `run()` termination wait

**Acceptance:** SIGINT/SIGTERM complete shutdown; no detached background tasks

#### Phase 4: Validated ID Hardening (Priority 1, Items 4-6)

**Files:**
- `crates/twerk-core/src/id/common.rs`
- `crates/twerk-core/src/id/job_id.rs`
- `crates/twerk-core/src/id/trigger_id.rs`
- `crates/twerk-core/src/domain/endpoint.rs`
- `crates/twerk-core/src/types/port.rs`
- `crates/twerk-core/src/types/progress.rs`
- `crates/twerk-core/src/types/retry_limit.rs`
- `crates/twerk-core/src/types/task_count.rs`
- `crates/twerk-core/src/types/task_position.rs`

**Changes:**
- Remove `Default` from validated IDs
- Replace infallible `From` with `TryFrom` or named constructors
- Privatize `Endpoint::new_unchecked()`
- Store parsed value in Endpoint instead of re-parsing in `as_url()`
- Remove infallible constructors from all primitive wrappers

**Acceptance:** Production code cannot construct invalid IDs/wrappers without explicit escape hatch

---

## Discovered Issues

1. **Dolt connectivity issues** - Multiple previous polecats (dust, fury) reported Dolt database connection failures with project ID mismatch. This infrastructure issue may block bead creation/closure.
2. **Worktree not initialized** - The maestro/twerk worktree has no git checkout. Previous polecats used brahmin's worktree as reference.
3. **Stale QA assets** - `qa/` directory still uses `$TORK_OUTPUT` instead of `$TWERK_OUTPUT` in several files.

---

## Effort Estimates

| Phase | Scope | Estimated Complexity |
|-------|-------|---------------------|
| Phase 1: Cancellation | 3 files, moderate refactoring | Medium |
| Phase 2: Timeout | 4 files, multi-runtime | Medium-High |
| Phase 3: Shutdown | 3 files, lifecycle wiring | Medium |
| Phase 4: ID Hardening | 9+ files, many call sites | High (wide blast radius) |

**Recommendation:** Execute Phases 1-3 sequentially (they are interrelated). Phase 4 can be done in parallel or as a separate batch due to its wide scope.

---

## Suggested Bead Decomposition

| Bead | Title | Priority | Depends On |
|------|-------|----------|------------|
| tw-cancellation-truth | Fix cancellation to report Cancelled not Completed | P0 | - |
| tw-timeout-authority | Make timeout enforcement authoritative | P0 | - |
| tw-shutdown-lifecycle | Centralize shutdown lifecycle | P0 | tw-cancellation-truth, tw-timeout-authority |
| tw-id-hardening | Remove Default/infallible From from validated IDs | P1 | - |
| tw-endpoint-safety | Privatize Endpoint::new_unchecked, fix as_url panic | P1 | - |
| tw-wrapper-hardening | Fix primitive wrapper bypass construction | P1 | tw-id-hardening |
| tw-qa-fix | Fix $TORK_OUTPUT → $TWERK_OUTPUT in QA files | P2 | - |

---

## Verification Strategy

- **Unit tests:** Each phase must have targeted unit tests for the specific fix
- **Contract tests:** Cancellation and timeout need contract tests proving state transitions
- **Integration tests:** Docker/Podman/Shell cancellation/timeout behavior
- **Regression tests:** Property tests for ID/wrapper constructors preventing backsliding
- **Build gate:** All phases must pass `cargo build`, `cargo clippy`, `cargo test`
