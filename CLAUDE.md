# CLAUDE.md — vo-engine

**Version:** 4.0 (V2 Architecture)
**Language:** Rust (end-to-end)
**Model:** Single-Binary, Fjall-backed, FaaS Orchestrator
**Build:** `moon run :ci`

## What This System Is
`vo-engine` is the Indestructible Rust Orchestrator. It is a true single-binary engine (no Docker, no NATS, no Postgres) that provides:
1. **Durable Execution:** Event-Sourcing backed by `fjall` (LSM-Tree) for face-melting disk IO.
2. **FaaS Subprocesses:** Workflows are strictly compiled Rust binaries spawned via `tokio::process::Command` (no Wasm/Docker).
3. **The BEAM Model:** `ractor` manages lock-free workflow state machines and hibernates them to disk when waiting.
4. **Visibility:** An embedded Dioxus WASM UI for n8n-style real-time graphs.

## Core V2 Architecture Rules (Must Read: `docs/adr/v2/`)
1. **Strictly Rust Binaries:** Workflows and Tasks are written using the `vo-sdk` and compiled to raw binaries. The engine discovers them via `./binary --graph` and executes them via `./binary --execute-node <name>`.
2. **FD3 / FD4 IPC:** The Engine NEVER uses `stdout` for state. It pipes input JSON to the child via FD3, and reads output JSON from FD4.
3. **Group Commits:** Actors NEVER write to `fjall` directly. All events are sent to the `DbWriterActor` to be batch-committed to prevent SSD lock contention.
4. **AI-Native:** CLI interfaces (`vo-cli history --json`) and definition schemas must output strict JSON intended for consumption by autonomous AI agents.

## Project Structure
| Crate | Purpose |
|-------|---------|
| `vo-common` | Shared types (`WorkflowEvent`, `InstanceId`) |
| `vo-core` | Minimal core types |
| `vo-actor` | `ractor` state machines (DAGs, FSMs, Procedural), Hibernation, and Subprocess Execution |
| `vo-storage` | `fjall` wrapper (`events`, `instances`, `timers` partitions) + `DbWriterActor` |
| `vo-api` | `axum` HTTP server (Webhook triggers, SSE telemetry) |
| `vo-cli` | Agent-first CLI (`vo-cli history`, `vo-cli check`) |
| `vo-sdk` | The developer macro crate (`#[vo_task]`, `Dag::new()`) |
| `vo-ui` | Dioxus WASM visual dashboard (Ported from Oya) |

## Development & AI Guidelines
1. **Zero External DBs:** Never introduce dependencies on Redis, Postgres, or NATS.
2. **Zero Wasm execution:** The engine executes OS binaries. Wasm is strictly for the UI.
3. **At-Most-One Actor:** The engine guarantees exactly one active `ractor` instance per workflow ID at any time.
4. **No Cargo Commands:** Ensure all checks run via `moon run :ci`.

## Go-skill Pipeline (for implementing new features)
```
STATE 1 → rust-contract (synthesize from code, not bd)
STATE 2 → test-reviewer
STATE 3 → functional-rust
STATE 4 → Moon Gate (moon run :ci)
STATE 4.5 → qa-enforcer
STATE 4.6 → QA review
STATE 5 → red-queen (adversarial)
STATE 5.5 → black-hat-reviewer
STATE 5.7 → kani-justification
STATE 6 → repair loop
STATE 7 → architectural-drift
STATE 8 → jj git push --bookmark main
```

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:b9766037 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

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
   bd dolt push
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
