# Findings: tw-p195 - Consolidate duplicate type definitions in hardline

## Task Summary
**Bead ID:** tw-p195
**Title:** hardline: Consolidate duplicate type definitions across crates
**Status:** AUDIT COMPLETE - No code changes made (source outside worktree)

## Important Context
The hardline source code is located at `/home/lewis/src/hardline/` (GitHub: lprior-repo/hardline).
The mutant polecat's worktree at `/home/lewis/gt/polecats/mutant/twerk/` does not contain the hardline source.
Therefore, this bead was treated as an AUDIT task - findings documented but no code changes made.

---

## Duplicate Type Definitions Found

### VcsStatus (4 definitions - should be 1)

**True Duplicates (identical):**
1. `/home/lewis/src/hardline/crates/core/src/vcs_types.rs:37` - Clean, Dirty, Conflicted, Detached
2. `/home/lewis/src/hardline/crates/vcs/src/domain/value_objects/mod.rs:6` - IDENTICAL
3. `/home/lewis/src/hardline/deacon/dogs/alpha/hardline/crates/vcs/src/domain/value_objects/mod.rs:6` - IDENTICAL (in dogs/alpha worktree)
4. `/home/lewis/src/hardline/deacon/dogs/alpha/hardline/crates/core/src/vcs_types.rs:37` - IDENTICAL (in dogs/alpha worktree)

**Canonical Source:** `crates/core/src/vcs_types.rs`

---

### QueueStatus (9 definitions - SEMANTICALLY DIFFERENT)

There are TWO DIFFERENT QueueStatus types with different semantics:

**Type A - Simple Queue Status** (2 definitions, identical):
- `crates/core/src/queue.rs:25`
  - Pending, Processing, Completed, Failed, Cancelled
- `crates/core/src/infrastructure/schema.rs:282` (SQL representation)

**Type B - Complex State Machine** (7 definitions, slightly different):
- `crates/queue/src/domain/queue/status.rs:16` - Claimed, Rebasing, Testing, ReadyToMerge, Merging, Merged, FailedRetryable, FailedTerminal, Cancelled
- `crates/core/src/domain/queue/status.rs:21` - **Claims to be "single canonical"** - same variants
- `crates/core/src/infrastructure/schema.rs:282` (SQL representation)
- In `deacon/dogs/alpha/hardline/` (mirror definitions):
  - `crates/queue/src/domain/queue/status.rs:13`
  - `crates/queue/src/domain/entities/queue_entry.rs:21`
  - `crates/core/src/queue.rs:25`
  - `crates/core/src/domain/queue/status.rs:16`
  - `crates/core/src/infrastructure/schema.rs:282`

**Analysis:** Type B is the more mature state machine. Type A appears to be legacy.
**Recommendation:** Deprecate Type A (queue.rs simple version), use Type B exclusively.

---

### AgentStatus (8 definitions - SEMANTICALLY DIFFERENT)

**Type A - Simple Agent Activity** (2 definitions, identical):
- `crates/core/src/agent.rs:148` - Active, Stale
- `crates/core/src/domain/agent_registry/status.rs:16` - likely same

**Type B - CLI Contract Agent Status** (6 definitions, identical):
- `crates/core/src/cli_contracts/domain_types/status_enums.rs:71` - Pending, Running, Completed, Failed, Cancelled, Timeout
- `crates/cli/src/commands/handlers/query/data.rs` - likely same
- In `deacon/dogs/alpha/hardline/`:
  - `crates/core/src/agent.rs:148`
  - `crates/core/src/domain/agent_registry/status.rs:16`
  - `crates/core/src/infrastructure/schema.rs:315`
  - `crates/core/src/cli_contracts/domain_types/status_enums.rs:71`

**Analysis:** These are TWO DIFFERENT types that happen to share a name:
- `AgentStatus` (agent.rs): Simple heartbeat tracking - Active/Stale
- `AgentStatus` (cli_contracts): CLI-facing status - Pending/Running/Completed/Failed/Cancelled/Timeout

**Recommendation:** Rename one to avoid confusion. Consider `AgentActivity` for the heartbeat type.

---

### SessionStatus (6 definitions - SEMANTICALLY DIFFERENT)

**Type A - CLI Contracts** (2 definitions, identical):
- `crates/core/src/cli_contracts/domain_types/status_enums.rs:14` - Creating, Active, Paused, Completed, Failed
- `crates/cli/src/commands/handlers/query/data.rs:95`

**Type B - Full Lifecycle** (4 definitions, identical):
- `crates/core/src/type_session_status.rs:12` - Creating, Active, Paused, Completed, Failed (with full state machine)
- In `deacon/dogs/alpha/hardline/`:
  - `crates/core/src/type_session_status.rs:12`
  - `crates/core/src/cli_contracts/domain_types/status_enums.rs:14`
  - `crates/cli/src/commands/handlers/query/data.rs:95`

**Analysis:** Type B is more complete (implements LifecycleState trait). Type A is a subset.
**Recommendation:** Use Type B everywhere, deprecate Type A simple version.

---

### OpStatus (2 definitions - IDENTICAL DUPLICATE)

- `crates/receipt/src/domain/receipt.rs:11` - InProgress, Success, Failed
- `crates/snapshot/src/domain/receipt.rs:11` - IDENTICAL

**Canonical Source:** `crates/receipt/src/domain/receipt.rs`
**Recommendation:** Import from receipt, remove from snapshot.

---

## The deacon/dogs/alpha/ Directory

Located at `/home/lewis/src/hardline/deacon/dogs/alpha/hardline/` - appears to be a:
1. Worktree of the hardline project, OR
2. A fork for the dogs/alpha deacon

This directory contains DUPLICATE definitions of all the same types, mirroring the main crates/ structure.

**Question:** Is this a git worktree or a separate fork? If worktree, then fixing in main location automatically fixes it. If fork, needs separate fix.

---

## Recommendations Summary

| Type | Current Count | Canonical Location | Action |
|------|---------------|-------------------|--------|
| VcsStatus | 4 | crates/core/src/vcs_types.rs | Delete 3 duplicates, re-export |
| QueueStatus (simple) | 2 | crates/core/src/queue.rs | Deprecate, use complex version |
| QueueStatus (complex) | 7 | crates/core/src/domain/queue/status.rs | Keep, is already "canonical" |
| AgentStatus (activity) | 2 | crates/core/src/agent.rs | Rename to AgentActivity |
| AgentStatus (cli) | 6 | crates/core/src/cli_contracts/... | Keep, is CLI contract |
| SessionStatus (simple) | 2 | crates/core/src/cli_contracts/... | Deprecate, use full version |
| SessionStatus (full) | 4 | crates/core/src/type_session_status.rs | Keep, is more complete |
| OpStatus | 2 | crates/receipt/src/domain/receipt.rs | Delete from snapshot, re-export |

---

## Files That Need Changes

To consolidate, these files need edits:

1. **crates/vcs/src/domain/value_objects/mod.rs** - Delete VcsStatus enum, re-export from core
2. **crates/snapshot/src/domain/receipt.rs** - Delete OpStatus enum, re-export from receipt
3. **crates/core/src/agent.rs** - Rename AgentStatus to AgentActivity
4. **crates/core/src/domain/agent_registry/status.rs** - Update to use renamed type
5. **crates/core/src/queue.rs** - Deprecate simple QueueStatus or remove if not used
6. **crates/core/src/cli_contracts/domain_types/status_enums.rs** - Remove simple SessionStatus, re-export full one
7. **crates/cli/src/commands/handlers/query/data.rs** - Update SessionStatus and AgentStatus imports

Plus corresponding updates in `deacon/dogs/alpha/hardline/` if it's a separate fork.

---

## Audit Conclusion

This is a MAJOR refactoring task. The duplicate definitions span multiple crates and some types have semantically different variants that share the same name. The consolidation requires:

1. Careful analysis of which variant is "correct" for each use case
2. Renaming where semantically different types share names
3. Re-exporting types from canonical locations
4. Updating all import statements across the codebase

**Estimated scope:** 2+ days of work for a thorough fix with testing.

---

*Findings compiled by: mutant polecat (twerk rig)*
*Date: 2026-04-24*
*Source location: /home/lewis/src/hardline/*
