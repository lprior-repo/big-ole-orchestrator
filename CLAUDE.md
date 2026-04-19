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
| `vo-executor` | Execute-node error handling, timeout enforcement, and retry policies |

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

<!-- BEGIN BEADS INTEGRATION v:1 profile:full hash:f65d5d33 -->
## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Git-friendly: Dolt-powered version control with native sync
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

### Quick Start

**Check for ready work:**

```bash
bd ready --json
```

**Create new issues:**

```bash
bd create "Issue title" --description="Detailed context" -t bug|feature|task -p 0-4 --json
bd create "Issue title" --description="What this issue is about" -p 1 --deps discovered-from:bd-123 --json
```

**Claim and update:**

```bash
bd update <id> --claim --json
bd update bd-42 --priority 1 --json
```

**Complete work:**

```bash
bd close bd-42 --reason "Completed" --json
```

### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

### Workflow for AI Agents

1. **Check ready work**: `bd ready` shows unblocked issues
2. **Claim your task atomically**: `bd update <id> --claim`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue:
   - `bd create "Found bug" --description="Details about what was found" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`

### Quality
- Use `--acceptance` and `--design` fields when creating issues
- Use `--validate` to check description completeness

### Lifecycle
- `bd defer <id>` / `bd supersede <id>` for issue management
- `bd stale` / `bd orphans` / `bd lint` for hygiene
- `bd human <id>` to flag for human decisions
- `bd formula list` / `bd mol pour <name>` for structured workflows

### Auto-Sync

bd automatically syncs via Dolt:

- Each write auto-commits to Dolt history
- Use `bd dolt push`/`bd dolt pull` for remote sync
- No manual export/import needed!

### Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Link discovered work with `discovered-from` dependencies
- ✅ Check `bd ready` before asking "what should I work on?"
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems

For more details, see README.md and docs/QUICKSTART.md.

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
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

## ⚠️ Dolt Database Safety (CRITICAL)

The `bd` CLI uses Dolt for issue tracking. The remote at DoltHub (`priorlewis43/veloxide-database`) is the **SOURCE OF TRUTH**.

### Database Location
- **Remote**: `priorlewis43/veloxide-database` on DoltHub
- **Local**: `.beads/dolt/` (working set only)

### ⚠️ NEVER DO THESE

1. **NEVER `rm -rf .beads/dolt`** without first verifying remote has data
2. **NEVER `dolt init`** on an existing project - this creates fresh empty repo
3. **NEVER `dolt clone`** over existing local without backing up first
4. **NEVER `dolt push --force`** to remote

### If Local Database Gets Corrupted

```bash
# ⚠️ STOP - The remote is the source of truth!
# DO NOT delete remote

# Backup current state (even if broken)
cp -r .beads/dolt /tmp/dolt-backup

# Remove corrupted local
rm -rf .beads/dolt

# Clone fresh from remote
dolt clone priorlewis43/veloxide-database
mv veloxide-database dolt

# Start server
bd dolt start

# Verify data is there
bd list --json
```

### If `dolt pull` Says "No Common Ancestor"

This means local and remote have diverged. The remote is almost always correct:
```bash
# Check what's in remote
dolt log remotes/origin/main | head -20

# If remote looks correct, overwrite local:
rm -rf .beads/dolt
dolt clone priorlewis43/veloxide-database
mv veloxide-database dolt
```

### Verify Remote After Every Push

After `bd dolt push`, ALWAYS verify at:
https://www.dolthub.com/repositories/priorlewis43/veloxide-database

<!-- END BEADS INTEGRATION -->
