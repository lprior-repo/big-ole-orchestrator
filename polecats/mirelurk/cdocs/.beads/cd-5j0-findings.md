# Findings: cd-5j0 Phantom Hook

## Issue
Hook references `cd-5j0: ARCH-DRIFT: drift detection wave3-3` but bead does not exist in database.

## Evidence
- `bd show cd-5j0` → "no issue found matching"
- `bd list --status=hooked` → "No issues found"
- `gt hook` → shows cd-5j0 as hooked
- Pattern matches `tw-pgom: Hook/DB inconsistency: cd-ypo shows as hooked but does not exist`

## Root Cause
The hook table in Dolt references a bead ID that was never persisted or was deleted.

## Resolution
This is a system bug - phantom hook. No code work possible. Exit cleanly.
