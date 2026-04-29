# vel-fix2 Findings

## Status: CANNOT COMPLETE - Dolt Database Unavailable

## Issue
Attempted to claim and work on bead `vel-fix2` as instructed, but:

1. **Dolt Database Unavailable**: The shared Dolt SQL server (port 3307) has a PROJECT IDENTITY MISMATCH error. The town-level metadata.json specifies `project_id: e73a37e0-a1e9-417b-940b-bce186abda73` but the server reports database project IDs of `c1279168...` and `af445fe7...` (different on each attempt).

2. **Bead `vel-fix2` Not Found**: Searched all databases (`veloxide-database`, `twerk`, and all databases in `/home/lewis/gt/.dolt-data/`) - the issue `vel-fix2` does not exist anywhere.

## Root Cause
The Dolt server on port 3307 was started with `--data-dir /home/lewis/gt/.beads/dolt` but the town-level metadata.json has mismatched configuration. The server appears to be cycling through databases or has configuration drift.

## Investigation Steps Run
- `bd dolt status` - Server reports multiple .doltcfg directories detected
- `dolt sql -q "SHOW tables"` on veloxide-database - Works locally, shows 26 tables
- Searched for `vel-fix2` in issues, wisps, issue_snapshots, interactions - Not found
- `gt prime --hook` - Fails with "embeddeddolt: init schema: creating schema_migrations table: Error 1105: no database selected"

## Resolution Required
- Dolt server needs to be reconfigured or restarted with correct --doltcfg-dir
- Or town-level metadata.json needs updated project_id to match server
- The bead `vel-fix2` may need to be created first if it doesn't exist

## Commands That Need Manual Fix
```bash
# Check rigs.json missing warning
ls -la /home/lewis/gt/rigs.json  # Likely missing

# Dolt server was started with wrong data-dir
# Server PID 1845809: dolt sql-server -H 127.0.0.1 -P 3307 --data-dir /home/lewis/gt/.beads/dolt
# But databases are in /home/lewis/gt/.dolt-data/
```

## Chrome Polecat Session
- Session: 1d92bb45-116b-4fa6-8983-4690cbde5f26
- Rig: veloxide
- Role: polecat
- Cannot claim bead due to database unavailability
- Exiting cleanly per user instruction (no gt done since db is broken)
