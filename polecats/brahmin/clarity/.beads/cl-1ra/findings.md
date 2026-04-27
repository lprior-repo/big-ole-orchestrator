# Findings: cl-1ra - GO-PLAN: clarity task 15

## Issue
GO-PLAN: clarity task 15 - Plan improvements for clarity

## Analysis Method
- Examined clarity rig and project structure
- Ran `bd doctor` for health checks
- Ran `bd doctor --check=conventions` for convention checks
- Reviewed existing open issues
- Examined recent git history and project architecture

## Current State of Clarity Project

### Project Overview
- **Type**: Dioxus 0.7 fullstack web application (clarity-web)
- **Location**: `/home/lewis/src/clarity/`
- **Git Remote**: https://github.com/lprior-repo/clarity.git
- **Current Branch**: main
- **Recent Focus**: Functional-rust refactoring (eliminating unwrap/unwrap_or violations)

### Project Structure (clarity-web/src/)
```
app/        - Main application code
bin/        - Binary crates
components/ - Reusable UI components
config/     - Configuration
domain/     - Domain logic
hooks/      - Custom React-like hooks
intent/     - Intent handling
kirk/       - Sub-project/module
lattice/    - Lattice-based components
pages/      - Page components
pme/        - Sub-project/module
providers/  - Context providers
storage/    - Storage layer (redb database)
ui/         - UI primitives
```

### Rig Health (bd doctor)
- **Total Checks**: 66 passed, 6 warnings, 0 errors
- **Status**: OPERATIONAL with warnings

### Warnings from bd doctor

1. **Database Version Metadata** (MEDIUM)
   - Missing version metadata in database
   - Fix: `bd doctor --fix`

2. **Repo Fingerprint** (MEDIUM)
   - Missing repo fingerprint metadata
   - Fix: `bd doctor --fix`

3. **CLI Version** (LOW)
   - Running v1.0.0, latest is v1.0.2
   - Fix: `curl -fsSL https://raw.githubusercontent.com/steveyegge/beads/main/scripts/install.sh | bash`

4. **Project Identity** (HIGH)
   - Missing `project_id` in metadata.json and database
   - Without project identity, cross-project data leakage cannot be detected
   - Fix: `bd doctor --fix`

5. **Project Gitignore** (MEDIUM)
   - No project .gitignore found
   - Dolt/credential files may be committed accidentally
   - Fix: `bd init` or `bd doctor --fix`

6. **Agent Documentation** (HIGH)
   - No AGENTS.md or CLAUDE.md found
   - AI agents lack workflow guidance
   - Fix: `bd onboard`

### Open Issues Analysis
9 open issues:
- 5x BLACKHAT (batches 1, 3, 4, 5, 8) - Adversarial security testing
- 4x ARCH-DRIFT (batches 1, 2, 4) - Architectural drift analysis

## Planned Improvements for Clarity

### 1. Project Identity (HIGH PRIORITY)
**Issue**: Missing `project_id` in metadata.json and database
**Impact**: Cannot detect cross-project data leakage; bd doctor warning
**Fix**: Run `bd doctor --fix` to generate and backfill project identity

### 2. Agent Documentation (HIGH PRIORITY)
**Issue**: No AGENTS.md or CLAUDE.md found in project
**Impact**: AI agents lack workflow guidance for this project
**Fix**: Run `bd onboard` to create AGENTS.md with workflow guidance

### 3. Project Gitignore (MEDIUM PRIORITY)
**Issue**: No project `.gitignore` found
**Impact**: Dolt/credential files may be committed accidentally
**Fix**: Create `.gitignore` with Dolt patterns or run `bd init`

### 4. Database Metadata Fixes (MEDIUM PRIORITY)
**Issue**: Missing version metadata and repo fingerprint
**Impact**: Incomplete bd doctor health profile
**Fix**: Run `bd doctor --fix`

### 5. CLI Upgrade (LOW PRIORITY)
**Issue**: Running bd v1.0.0, latest is v1.0.2
**Impact**: Missing latest features/fixes
**Fix**: Run the install script to upgrade

## Recommendations Summary

1. **Immediate**: Run `bd onboard` to create AGENTS.md - critical for agent effectiveness
2. **Immediate**: Run `bd doctor --fix` to fix project identity and orphaned dependencies
3. **Soon**: Create project `.gitignore` with standard Dolt patterns
4. **Optional**: Upgrade bd CLI to v1.0.2

## Exit Status
This was a planning/audit task - no code changes required.
All findings documented above.

**Session**: Fri Apr 24 2026
**Polecat**: brahmin
**Rig**: clarity