# Agent Instructions

This project uses **bd** (beads) for issue tracking. Run `bd onboard` to get started.

## Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work atomically
bd close <id>         # Complete work
```

## Beads Setup (REMOTE-BACKED)

This project uses **bd with Dolt remote sync** to prevent data loss.

### Database
- **Remote**: `priorlewis43/veloxide-database` on DoltHub
- **Local**: `.beads/dolt/` (working set only)
- **Server mode**: `bd dolt start` runs a local Dolt server (port in `.beads/dolt-server.port`)

### CRITICAL: Never Lose Data Again

**BEFORE ENDING EVERY SESSION:**

```bash
bd dolt push  # Push to DoltHub BEFORE anything else
```

**START OF EVERY SESSION:**

```bash
bd dolt pull  # Pull latest from DoltHub
bd ready      # Get ready work
```

### Backup Verification

After `bd dolt push`, verify at: https://www.dolthub.com/repositories/priorlewis43/veloxide-database

### If Database Gets Corrupted or Fresh

**⚠️ CRITICAL: The remote DoltHub database is the SOURCE OF TRUTH. NEVER delete or overwrite it.**

```bash
# Stop current server
bd dolt stop

# Remove ONLY the local corrupted state
rm -rf .beads/dolt

# Clone FRESH from remote (creates new local from remote)
dolt clone priorlewis43/veloxide-database

# Rename to dolt for bd to find
mv veloxide-database dolt

# Start server
bd dolt start

# Verify
bd list --json
```

### ⚠️ Dolt Database Safety Rules

1. **Remote is source of truth** - Never `rm -rf` the remote
2. **Always verify before clone** - If local exists, BACK IT UP first
3. **Never `dolt init` on existing project** - This creates fresh empty repo
4. **If `bd dolt push` fails with "no common ancestor"** - Local and remote diverged. Fetch remote and investigate before force-pushing
5. **After ANY `bd dolt push`** - Verify at https://www.dolthub.com/repositories/priorlewis43/veloxide-database

### Recovering from Diverged Local/Remote

If `dolt pull` fails with "no common ancestor", the local was force-pushed to diverged:
```bash
# Save any local work
cp -r .beads/dolt /tmp/dolt-backup

# Remove diverged local
rm -rf .beads/dolt

# Clone fresh from remote (remote is always correct)
dolt clone priorlewis43/veloxide-database
mv veloxide-database dolt

# Manually check /tmp/dolt-backup for any commits not in remote
```

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags** with file operations to avoid hanging on confirmation prompts.

Shell commands like `cp`, `mv`, and `rm` may be aliased to include `-i` (interactive) mode on some systems, causing the agent to hang indefinitely waiting for y/n input.

**Use these forms instead:**
```bash
# Force overwrite without prompting
cp -f source dest           # NOT: cp source dest
mv -f source dest           # NOT: mv source dest
rm -f file                  # NOT: rm file

# For recursive operations
rm -rf directory            # NOT: rm -r directory
cp -rf source dest          # NOT: cp -r source dest
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

**MANDATORY BEADS SYNC at end of EVERY session:**

```bash
bd dolt push  # Push beads to DoltHub - this is NON-NEGOTIABLE
```

**Full workflow:**

```bash
# START OF SESSION
bd dolt pull  # Pull latest from DoltHub
bd ready      # Find available work

# DURING SESSION
bd create "Issue" -p 1 --json
bd update <id> --claim --json
bd close <id> --reason "Done" --json

# END OF SESSION - THIS IS MANDATORY
bd dolt push  # ALWAYS run this before exiting
```

**CRITICAL:**
- `bd dolt push` syncs issues to DoltHub (source of truth)
- `git push` syncs code separately
- NEVER exit a session without `bd dolt push` - issues will be lost
- If push fails, fix and retry until it succeeds

<!-- END BEADS INTEGRATION -->
