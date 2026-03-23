# CLAUDE.md — wtf-engine

**Version:** 3.0
**Language:** Rust (end-to-end)
**Model:** Deterministic Event-Sourced Replay

## What This System Is

wtf-engine is a durable execution runtime for long-lived workflows (payments, data pipelines, approval chains, ETL). It guarantees **no transition is ever lost** — if the process crashes mid-execution, it replays the NATS JetStream event log and arrives at exactly the correct state.

## Architecture

```
Layer 1: Control Plane (Dioxus WASM) — Design Mode, Simulate Mode, Monitor Mode
Layer 2: Execution Engine (Ractor + axum) — MasterOrchestrator, WorkflowInstance actors
Layer 3: Data Plane (NATS JetStream + KV) — event log, materialized view, sled snapshots
```

## Crates

| Crate | LOC | Purpose |
|-------|-----|---------|
| `wtf-common` | 690 | Shared types: `WorkflowEvent`, `InstanceId`, `RetryPolicy` |
| `wtf-core` | 44 | Minimal core types |
| `wtf-actor` | 3,896 | Ractor actors: MasterOrchestrator, FsmActor, DagActor, ProceduralActor |
| `wtf-storage` | 1,362 | NATS JetStream + KV wrappers, sled snapshot store |
| `wtf-worker` | 1,334 | Activity worker SDK |
| `wtf-api` | 1,786 | axum HTTP server, SSE, ingestion |
| `wtf-cli` | 996 | `wtf serve`, `wtf lint`, `wtf admin` |
| `wtf-linter` | 1,968 | Procedural workflow static analysis (6 rules) |
| `wtf-frontend` | 27,145 | Dioxus WASM dashboard |

**Total: ~39,221 Rust source lines, ~3,600 test lines**

## Three Execution Paradigms (ADR-017)

- **FSM** (`wtf-actor/src/fsm/`) — payment flows, order state, explicit named transitions
- **DAG** (`wtf-actor/src/dag/`) — pipelines, parallel fan-out/fan-in
- **Procedural** (`wtf-actor/src/procedural/`) — conditional logic, human loops, `ctx.activity()` checkpoint model

## Linter Rules (ADR-020)

All 6 rules implemented in `wtf-linter`:

| Rule | File | Status |
|------|------|--------|
| WTF-L001 non-deterministic-time | `l001_time.rs` | LANDED |
| WTF-L002 non-deterministic-random | `rules.rs` | ✅ Implemented |
| WTF-L003 direct-async-io | `l003_direct_io.rs` | ✅ Implemented |
| WTF-L004 ctx-in-closure | `l004.rs` | ✅ Implemented |
| WTF-L005 tokio-spawn | `l005.rs` | ✅ Implemented |
| WTF-L006 std-thread-spawn | `l006.rs` | ✅ Implemented |

## Running Tests

```bash
# All tests (NATS must be running in Docker)
cargo test --workspace

# Specific crate
cargo test -p wtf-actor
cargo test -p wtf-storage
cargo test -p wtf-linter

# With output
cargo test --workspace -- --nocapture
```

## NATS Connection

NATS is running in Docker:
```bash
docker ps | grep nats
# wtf-nats-test  nats:2  "/nats-server -js"  4222/tcp
```

Test connection:
```bash
cargo run -p wtf-storage --bin nats_connect_test
```

## Bead Tracking (jj-first)

Beads are tracked via **jj commits** — each feature/refactor gets its own jj commit.
The `.beads/` directory is the **Dolt/bd database** (not bead artifacts).

### Go-skill Pipeline (for implementing new features)
```
STATE 1 → rust-contract (synthesize from code, not bd)
STATE 2 → test-reviewer
STATE 3 → functional-rust
STATE 4 → Moon Gate (cargo check, cargo test, cargo clippy)
STATE 4.5 → qa-enforcer
STATE 4.6 → QA review
STATE 5 → red-queen (adversarial)
STATE 5.5 → black-hat-reviewer
STATE 5.7 → kani-justification
STATE 6 → repair loop
STATE 7 → architectural-drift
STATE 8 → jj git push --bookmark main
```

## ADRs

Key architectural decisions in `docs/adr/`:
- ADR-013: NATS JetStream as event log
- ADR-014: NATS KV materialized view
- ADR-015: Write-ahead guarantee
- ADR-016: Deterministic replay model
- ADR-017: Three execution paradigms
- ADR-018: Dioxus as compiler
- ADR-019: Snapshot recovery
- ADR-020: Procedural workflow linter

## Key Files

- `crates/wtf-common/src/events/mod.rs` — `WorkflowEvent` enum (19 variants)
- `crates/wtf-actor/src/procedural/context.rs` — `WorkflowContext` with `ctx.activity()`, `ctx.now()`, `ctx.sleep()`, `ctx.random_u64()`
- `crates/wtf-storage/src/journal.rs` — `append_event` (write-ahead publish+ack)
- `crates/wtf-storage/src/replay.rs` — `replay_events`, `create_replay_consumer`
- `crates/wtf-api/src/routes.rs` — HTTP endpoint definitions

## Known Issues

1. **7 journal_test failures** — tests assert wrong status codes (missing Extension setup)
2. **wtf-cli and wtf-worker have NO tests** — 0 test lines each
