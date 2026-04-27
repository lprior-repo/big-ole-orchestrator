# vel-fix1 Findings

## Status: BLOCKED - Dolt Database Infrastructure Failure

## Issue Summary
Cannot claim or work on bead vel-fix1 due to critical Dolt database infrastructure failure.

## Technical Details

### Problem
- **Project ID Mismatch**: Local metadata.json expects project ID `e73a37e0-a1e9-417b-940b-bce186abda73` but Dolt server serves database with project ID `af445fe7-feaa-48f5-b33b-258b66d93a10`
- **Database Exists but Not Accessible**: veloxide-database exists at `/home/lewis/gt/.beads/dolt/veloxide-database/` with all required tables (verified via direct access)
- **Multiple Dolt Servers Running**: PID 1845809 on port 3307, PID 1986857 on port 3308
- **bd commands fail**: All `bd` commands fail with "PROJECT IDENTITY MISMATCH" error

### Server Configuration
- Port 3307 server: `dolt sql-server -H 127.0.0.1 -P 3307 --data-dir /home/lewis/gt/.beads/dolt`
- Port 3308 server: `dolt sql-server -H 127.0.0.1 -P 3308` (no data-dir, using cwd)

### Database Verification
```bash
$ cd /home/lewis/gt/.beads/dolt/veloxide-database && dolt sql -q "show tables"
# Returns 24 tables including: issues, blocked_issues, comments, dependencies, labels, etc.
```

### Escalation Attempts
1. `bd update vel-fix1 --claim` - FAILED: PROJECT IDENTITY MISMATCH
2. `gt prime --hook` - FAILED: no database selected
3. `gt escalate` - FAILED: creating escalation bead also hits PROJECT IDENTITY MISMATCH

## Root Cause
The Dolt server on port 3307 was started with `--data-dir /home/lewis/gt/.beads/dolt` but is not properly serving the `veloxide-database` subdirectory. Instead it appears to be serving a different database (with project ID `af445fe7-feaa-48f5-b33b-258b66d93a10`).

## Resolution Required
1. Stop incorrect Dolt server(s)
2. Properly configure and start Dolt server to serve `/home/lewis/gt/.beads/dolt/veloxide-database`
3. Verify `bd list --json` works before polecat can resume work

## Affected Beads
- vel-fix1 (cannot claim)
- All other veloxide beads inaccessible via `bd` CLI

## Timestamp
2026-04-24T00:59 UTC

## Actor
veloxide/polecats/brahmin
