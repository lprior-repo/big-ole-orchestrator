# Findings: ha-zbd Bead Investigation

## Issue Status: CANNOT CLAIM - Issue Not Found

## Problem Description

The bead `ha-zbd` was requested to be claimed, but the Dolt database infrastructure is in an inconsistent state:

1. **Database Not Found**: The `bd` commands fail with "database 'ha' not found on Dolt server at 127.0.0.1:3307"

2. **Project ID Mismatch**: 
   - Worktree metadata expects project_id: `e73a37e0-a1e9-417b-940b-bce186abda73`
   - Database `ha` in embeddeddolt has project_id: `d76d58b6-bc5c-41f2-bcfd-0d342a4489a6`

3. **Data Discrepancy**: Earlier in this session, issue `ha-zbd` was visible with:
   - Title: "GO-IMPLEMENT: hardline implementation 15"
   - Status: in_progress
   - Assignee: hardline/polecats/brahmin
   - Issue type: task
   - Priority: 2
   
   But subsequent queries show 0 issues in the `ha` database.

4. **Dolt Server Configuration Issues**:
   - Multiple `.doltcfg` directories conflict (`/home/lewis/gt/.beads/.doltcfg` and `/home/lewis/gt/.beads/embeddeddolt/.doltcfg`)
   - Server intermittently shows different databases depending on data directory configuration
   - The `bd dolt start` command starts server with `--data-dir /home/lewis/gt/.beads/dolt` which doesn't contain the `ha` database

## Root Cause

The hardline worktree's `.beads/redirect` points to `/home/lewis/gt/.beads`, but:
- The main `.beads/dolt` data directory contains `veloxide-database` and `twerk`, not `ha`
- The `ha` database exists in `/home/lewis/gt/.beads/embeddeddolt/ha/`
- bd commands are configured to look for database "ha" but the server doesn't serve it consistently

## Investigation Performed

- Queried all databases (cdocs, ha, hardline, veloxide) - all show 0 issues except veloxide (3 issues with vel- prefix)
- Attempted multiple server configurations with different --data-dir and --doltcfg-dir options
- Renamed conflicting .doltcfg directory to allow server startup
- Server still exhibits inconsistent behavior showing different databases at different times

## Recommendation

The Dolt infrastructure needs repair before bead `ha-zbd` can be claimed:

1. Determine correct project_id for the hardline worktree
2. Either migrate `ha` database to correct location or update worktree metadata to match database
3. Resolve the multiple .doltcfg directory conflict
4. Verify issue `ha-zbd` exists in the correct database before attempting to claim

## Commands That Need to Pass

Before work can begin:
```bash
bd update ha-zbd --claim  # Must succeed
bd show ha-zbd           # Must show the issue
```

These currently fail due to infrastructure issues.
