# CLAUDE.md — Veloxide

**Version:** 5.0 (V2 Architecture)
**Language:** Rust (end-to-end)
**Model:** Single-Binary, Fjall-backed, FaaS Orchestrator
**Build:** `moon run :ci`

## What This System Is
Veloxide is the Indestructible Rust Orchestrator. It is a true single-binary engine (no Docker, no Postgres) that provides:
1. **Durable Execution:** Event-Sourcing backed by `fjall` (LSM-Tree) for face-melting disk IO.
2. **FaaS Subprocesses:** Workflows are strictly compiled Rust binaries spawned via `tokio::process::Command` (no Wasm/Docker).
3. **The BEAM Model:** `ractor` manages lock-free workflow state machines and hibernates them to disk when waiting.
4. **Visibility:** A Dioxus WASM frontend (`vo-frontend`) for n8n-style real-time graphs.

## Core V2 Architecture Rules (Must Read: `docs/adr/v2/`)
1. **Strictly Rust Binaries:** Workflows and Tasks are written using the `vo-sdk` and compiled to raw binaries. The engine discovers them via `./binary --graph` and executes them via `./binary --execute-node <name>`.
2. **FD3 / FD4 IPC:** The Engine NEVER uses `stdout` for state. It pipes input JSON to the child via FD3, and reads output JSON from FD4.
3. **Group Commits:** Actors NEVER write to `fjall` directly. All events are sent to the `DbWriterActor` to be batch-committed to prevent SSD lock contention.
4. **AI-Native:** CLI interfaces (`vo-cli history --json`) and definition schemas must output strict JSON intended for consumption by autonomous AI agents.

## Project Structure
| Crate | Purpose |
|-------|---------|
| `vo-types` | Core domain types (`WorkflowEvent`, `InstanceId`, `StepResult`, connectors) with proptest support |
| `vo-common` | Shared types and utilities |
| `vo-core` | Minimal core types |
| `vo-actor` | `ractor` state machines (DAGs, FSMs, Procedural), Hibernation, and Subprocess Execution |
| `vo-storage` | `fjall` wrapper (`events`, `instances`, `timers` partitions) + `DbWriterActor` |
| `vo-api` | `axum` HTTP server (Webhook triggers, SSE telemetry) |
| `vo-cli` | Agent-first CLI (`vo-cli history`, `vo-cli check`) |
| `vo-ipc` | Inter-process communication — FD3/FD4 pipe protocol, envelope framing, subprocess I/O |
| `vo-worker` | NATS-based worker for distributed task execution |
| `vo-sdk` | The developer macro crate (`#[vo_task]`, `Dag::new()`) |
| `vo-sdk-macros` | Procedural macros backing `vo-sdk` attribute macros |
| `vo-frontend` | Dioxus WASM visual dashboard (graph UI, node panels) |
| `vo-linter` | Static analysis crate for linting workflow definitions (AST-based via `syn`) |
| `vel-k1t9` | Execute-node error handling, timeout enforcement, and retry policies |

## Development & AI Guidelines
1. **Zero External DBs:** Never introduce dependencies on Redis or Postgres. NATS is used exclusively by `vo-worker` for distributed task execution.
2. **Zero Wasm execution:** The engine executes OS binaries. Wasm is strictly for the UI (`vo-frontend`).
3. **At-Most-One Actor:** The engine guarantees exactly one active `ractor` instance per workflow ID at any time.
4. **No Cargo Commands:** Ensure all checks run via `moon run :ci`.
5. **FD3/FD4 IPC:** The Engine NEVER uses `stdout` for state. It pipes input JSON to the child via FD3, and reads output JSON from FD4 (see `vo-ipc`).

## Go-skill Pipeline (for implementing new features)
```
STATE 0  → Isolation & Calibration (bd claim, jj workspace)
STATE 1  → rust-contract (Design-by-Contract synthesis)
STATE 1.5 → test-planner (Testing Trophy, BDD, proptest, Kani)
STATE 1.7 → test-reviewer (Plan Inquisition — Mode 1)
STATE 2  → test-writer (TDD Red Phase — failing tests only)
STATE 3  → functional-rust (Implementation — make tests green)
STATE 4  → Moon Gate (moon run :quick/:test/:ci/:e2e)
STATE 4.5 → qa-enforcer (Smoke + integration + adversarial)
STATE 4.6 → QA Review (PASS/FAIL decision)
STATE 4.7 → test-reviewer (Suite Inquisition — Mode 2)
STATE 5  → red-queen (Adversarial coevolution)
STATE 5.5 → black-hat-reviewer (5-phase code review)
STATE 5.7 → Kani model checking (or written justification)
STATE 6  → Repair Loop (fix defects, return to STATE 4)
STATE 7  → architectural-drift (<300 lines, Scott Wlaschin DDD)
STATE 8  → Landing (jj rebase, git push, bd close)
```

<!-- BEGIN BEADS INTEGRATION v:1 profile:full hash:d4f96305 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Local-Only Mode

This project uses **local-only beads** with JSONL persistence. No remote Dolt sync.

- Beads are stored in `.beads/issues.jsonl`
- Each session auto-commits to local Dolt history
- No `bd dolt push` needed — beads persist locally

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - `moon run :ci`
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->
