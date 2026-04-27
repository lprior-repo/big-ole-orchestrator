# Findings: cl-kwu (Phantom Hook)

## Issue
Hooked bead `cl-kwu: GO-PLAN: clarity task 3` does not exist in the clarity Dolt database.

## Investigation Steps Run
1. `bd update cl-kwu --claim` → Error: no issue found matching "cl-kwu"
2. `gt dolt pull --db clarity` → Success (pulled 1 database)
3. `bd list --status=open` → 59 open issues, all `tw-*` prefix (twerk project), zero `cl-*` issues
4. `bd show cl-kwu` → Error: no issue found matching "cl-kwu"
5. `bd list --title="clarity"` → No issues found

## Worktree Status
- Path: `/home/lewis/gt/polecats/mirelurk/clarity`
- `rtk git status` → "Not a git repository"
- Directory contents: `.beads/`, `cdocs/`, `clarity/`, `hardline/`, `seshat/`, `twerk/`, `veloxide/`
- The worktree is NOT a proper git worktree - no `.git` file present

## Clarity Rig Root
- Path: `/home/lewis/gt/clarity/`
- Contents: `.beads/`, `polecats/` only (no actual source code)

## Root Cause
Phantom hook - the hook references a bead ID that was never persisted to the clarity Dolt database, or was deleted/renamed.

## Resolution
Closed as phantom hook. Escalated to witness for investigation of hook/DB inconsistency.
