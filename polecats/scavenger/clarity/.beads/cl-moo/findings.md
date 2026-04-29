# Findings: cl-moo (ARCH-DRIFT: drift detection wave3-14)

## Status: PHANTOM HOOK — no-changes

## Summary
Bead `cl-moo` does not exist in the clarity rig Dolt database. This is a phantom hook dispatch — the polecat was spawned with a hook referencing a non-existent bead ID.

## Evidence
- `bd show cl-moo` → "no issue found matching cl-moo"
- `bd update cl-moo --claim` → "Error resolving cl-moo: no issue found"
- `bd search "drift"` → Found `tw-a571` (ARCH-DRIFT: drift detection wave3-14) in town (tw) database, NOT in clarity
- Dolt is healthy (latency 0s, 8 connections, running since 01:45)
- This matches known pattern tracked by tw-lgct: "Phantom hook cl-fy2 referenced non-existent bead"

## Root Cause
The mayor dispatch system hooked a bead ID (`cl-moo`) that was never created in the clarity database. The arch-drift beads may have been created in the town database instead (tw-a571) or the hook ID was generated but the bead creation failed silently.

## Related Issues
- tw-lgct: Phantom hook cl-fy2 (same pattern, different polecat)
- tw-s7h4: Hooked bead cl-cm8 does not exist
- tw-pgom: Hook/DB inconsistency cd-ypo
