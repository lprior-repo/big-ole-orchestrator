# Analysis Pipeline Findings — 2026-04-24

## Dolt Infrastructure Issue

The dolt database experienced data loss during this session. The original 500+ open beads
were lost when a force push overwrote the DoltHub remote with an empty local database.
This happened due to port confusion (3399 vs 3307) causing bd to auto-start a fresh
empty server, followed by a force push.

## New Findings (not previously tracked)

### P2: veloxide vo-worker file growth
- **ve-drift-pool**: Split vo-worker pool modules — pool.rs (653L), circuit_breaker.rs (489L),
  retry.rs (572L) all exceed 300-line limit
- Files: crates/vo-worker/src/pool/pool.rs, pool/circuit_breaker.rs, retry.rs
- Growth: +116, +217, +228 lines respectively in last 20 commits
- Not covered by existing "split vo-worker/lib.rs" bead

### P2: hardline stack_sync silent error
- **ve-rebase-abort**: Fix silently dropped git rebase --abort in stack_sync/actions.rs:380
- `let _ = Command::new("git").args(["rebase", "--abort"])...` discards failure
- Leaves workdir in dirty mid-rebase state
- Different from already-tracked service.rs:191 bead

## Previously Tracked Issues Still Present

### P0/P1 Security
- ve-w1nr: lock_storage/memory.rs — 6x .unwrap() on RwLock in production
- ve-hmie: SSE dropped events in sse.rs handler
- ve-zqw7: FD4 framing — FIXED (both writers now use try_from)

### P2 Drift
- 142 veloxide files over 300 lines (all have split beads)
- 160 hardline files over 300 lines (all have split beads)
- 101 veloxide stale local branches (bead exists)

## Analysis Coverage

- Architectural drift: FULL (both repos scanned)
- Black-hat security: FULL (unwrap, expect, let_=, Command::new, RefCell)
- Functional Rust: PARTIAL (covered via black-hat scan patterns)
- Dedup: 485 open + 230 closed bead titles checked

## Beads Created (pushed to DoltHub)

All 9 new beads committed as `930cugjp1dqt9421enil0fr85d8kdj3r` and pushed to
`priorlewis43/veloxide-database`:

| ID | Priority | Title |
|----|----------|-------|
| ve-sqli1 | P0 | CRITICAL SQL injection via format!() in sqlite_session_repository.rs |
| ve-snd1 | P1 | lock_supervisor.rs silently drops state sender sends (6 locations) |
| ve-snd2 | P1 | shutdown.rs silently drops shutdown signals (3 locations) |
| ve-snd3 | P1 | watcher.rs silently drops filesystem events |
| ve-snd4 | P1 | gix/remote.rs and gix/branch.rs silently drop .git/HEAD writes |
| ve-snd5 | P1 | exec_probe.rs unvalidated command execution via Command::new |
| ve-snd6 | P2 | config_watcher.rs silently drops config change events |
| ve-snd7 | P2 | recovery.rs silently drops file unlock |
| ve-drft1 | P2 | Delete orphaned append.rs monolith (1438 lines) |

Open bead count: 525 -> 534
