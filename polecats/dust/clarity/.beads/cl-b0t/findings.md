# Findings for cl-b0t: Plan improvements for clarity

## Issue
GO-PLAN: clarity task 13 - Plan improvements for clarity

## Analysis Method
- Examined clarity rig structure
- Ran `bd doctor` for health checks
- Ran `bd doctor --check=conventions` for convention checks
- Reviewed existing open issues

## Current State of Clarity Rig

### Rig Configuration
- **Backend**: Dolt (server mode)
- **Database**: clarity
- **Beads location**: Redirects to `/home/lewis/gt/.beads`
- **Total issues**: 46 (19 open, 25 in progress, 2 closed)

### Open Issues Analysis
19 open issues, primarily:
- 8x BLACKHAT (adversarial security testing batches 1-8)
- 9x ARCH-DRIFT (architectural drift analysis batches 1-7 + wave3 series)
- 1x test issue (cl-l9z)
- 1x this planning issue (cl-b0t)

## Planned Improvements for Clarity

### 1. Project Identity (MEDIUM PRIORITY)
**Issue**: Missing `project_id` in `metadata.json`
**Impact**: Cannot detect cross-project data leakage
**Fix**: Run `bd doctor --fix` to generate and backfill project identity

### 2. Project Gitignore (MEDIUM PRIORITY)
**Issue**: No project `.gitignore` found
**Impact**: Dolt/credential files may be committed accidentally
**Fix**: Create `.gitignore` with Dolt patterns or run `bd init`

### 3. Agent Documentation (HIGH PRIORITY)
**Issue**: No AGENTS.md or CLAUDE.md found
**Impact**: AI agents lack workflow guidance
**Fix**: Run `bd onboard` to create AGENTS.md with workflow guidance

### 4. Federation Server (LOW PRIORITY)
**Issue**: Dolt SQL server not running for peer-to-peer sync
**Impact**: Federation features disabled (1 peer configured)
**Fix**: Start `dolt sql-server` in server mode if federation is needed

### 5. Orphaned Dependencies (MEDIUM PRIORITY)
**Issue**: 39 orphaned dependency references
**Impact**: Clutter in dependency graph, potential confusion
**Fix**: Run `bd doctor --fix` to remove orphaned dependencies
**Examples**: tw-1vs→tw-wisp-mr11, tw-1zi→tw-wisp-n8qn, etc.

## Recommendations

1. **Immediate**: Run `bd onboard` to create AGENTS.md - this is critical for agent effectiveness
2. **Soon**: Run `bd doctor --fix` to fix project identity and orphaned dependencies
3. **Create**: Project `.gitignore` with standard Dolt patterns
4. **Optional**: Enable federation server if peer sync is needed

## Exit Status
This was a planning/audit task - no code changes required.
All findings documented above.

**Session**: Fri Apr 24 2026
**Polecat**: dust
**Rig**: clarity