# Findings: Plan Improvements for Centralized-Docs

## Bead: cd-31v
## Title: GO-PLAN: cdocs task 7
## Date: 2026-04-24

---

## Executive Summary

The centralized-docs (cdocs rig) manages documentation for the veloxide project at `/home/lewis/gt/docs/`. This analysis identifies documentation issues and proposes improvements.

---

## Current Documentation Structure

```
docs/
├── adr/
│   └── v2/                    # Architecture Decision Records (ADR-001-v2 through ADR-047-v2)
├── contracts/                  # Contract documents for various components
├── qa-reports/                # QA reports
├── test-plans/                # Test plans
├── test-reviews/              # Test reviews
├── ADR_DEPENDENCY_GRAPH.md    # ADR dependency visualization
├── ADR_FREEZE_AUDIT.md        # Final audit before architecture freeze
├── API.md                     # API documentation (37.2KB)
├── IMPLEMENTATION_BUILD_ORDER.md  # Phase-by-phase implementation guide
└── VISION_AND_ARCHITECTURE.md    # EMPTY - needs content
```

---

## Issues Identified

### 1. VISION_AND_ARCHITECTURE.md is Empty (Critical)
- **Location**: `docs/VISION_AND_ARCHITECTURE.md`
- **Issue**: Contains only a placeholder comment: `# I am not running this command, just simulating the thought process.`
- **Impact**: No clear vision document for the project
- **Recommendation**: Write a comprehensive vision document covering:
  - Project mission and goals
  - Core architectural principles
  - Key design decisions
  - Target users and use cases

### 2. README.md References Non-Existent Architecture Doc
- **Location**: `README.md` line 35
- **Issue**: Links to `docs/architecture.md` which does not exist
- **Current content**: `- [Architecture](docs/architecture.md)`
- **Recommendation**: Either create `docs/architecture.md` or update the link to point to existing documentation (e.g., `docs/IMPLEMENTATION_BUILD_ORDER.md` or the ADR index)

### 3. ADR Numbering Gaps
- **Location**: `docs/adr/v2/`
- **Issue**: Missing sequential ADRs (e.g., no ADR-011-v2, ADR-044-v2, ADR-045-v2)
- **Found ADRs**: 001, 002, 003, 004, 005, 006, 007, 008, 009, 010, 012, 013, 014, 015, 016, 017, 018, 019, 020, 021, 022, 023, 024, 025, 026, 027, 028, 029, 030, 031, 032, 033, 034, 035, 036, 037, 038, 039, 040, 041, 042, 043, 046, 047
- **Missing**: 011, 044, 045
- **Impact**: Unclear if missing ADRs were deleted, renamed, or never created
- **Recommendation**: Investigate why these ADRs are missing and either create them or document their removal in ADR_FREEZE_AUDIT.md

### 4. Documentation Organization Issues
- **Issue**: No index or entry point for documentation
- **Impact**: Difficult for new contributors to understand where to start
- **Recommendation**: Create `docs/README.md` with:
  - Overview of documentation structure
  - Quick start guide
  - Links to key documents
  - ADR index

### 5. Stale Documentation References
- **Location**: Multiple files
- **Issue**: CLAUDE.md references `vo-engine` and `vo-ui` which do not exist in current workspace
- **Reference**: CLAUDE.md states crates like `vo-executor` exist, but Cargo.toml shows different crate names
- **Impact**: Confusing for AI agents reading these files
- **Recommendation**: Audit and update CLAUDE.md to match actual workspace structure

### 6. No Centralized Documentation Entry Point
- **Issue**: No single document that explains the overall project structure and how the docs are organized
- **Recommendation**: Create `docs/README.md` with navigation guidance

---

## Recommended Improvements (Priority Order)

### P0 - Critical
1. **Fix VISION_AND_ARCHITECTURE.md**: Write actual content explaining the project vision
2. **Fix README.md link**: Update architecture link from `docs/architecture.md` to valid documentation

### P1 - High
3. **Investigate missing ADRs (011, 044, 045)**: Determine if they should be created or formally removed
4. **Create docs/README.md**: Provide navigation and overview for the documentation system

### P2 - Medium
5. **Audit CLAUDE.md**: Update references to match actual workspace crate names
6. **Create ADR index document**: List all ADRs with brief descriptions and status

### P3 - Low (Nice to have)
7. **Add architecture diagrams**: Mermaid diagrams where helpful (some exist in ADR_DEPENDENCY_GRAPH.md)
8. **Cross-reference ADRs**: Ensure all ADRs reference related documents

---

## Workspace Crate Reality Check

According to Cargo.toml, current workspace crates are:
- vo-types
- vo-storage
- vo-api
- vo-cli
- vo-worker
- vo-frontend
- vo-linter
- vo-actor
- vo-core
- vo-common
- vo-ipc
- vo-sdk

Note: CLAUDE.md mentions `vo-executor` and `vo-sdk-macros` which don't appear in Cargo.toml workspace members.

---

## Files Analyzed

- `/home/lewis/gt/docs/VISION_AND_ARCHITECTURE.md` - Empty
- `/home/lewis/gt/docs/ADR_DEPENDENCY_GRAPH.md` - Good structure
- `/home/lewis/gt/docs/ADR_FREEZE_AUDIT.md` - Comprehensive audit
- `/home/lewis/gt/docs/IMPLEMENTATION_BUILD_ORDER.md` - Detailed phases
- `/home/lewis/gt/docs/API.md` - Large (37KB), may need review
- `/home/lewis/gt/README.md` - Has broken link
- `/home/lewis/gt/CLAUDE.md` - Has stale references

---

## Next Steps

1. File new beads for each P0 and P1 issue
2. Assign improvements to relevant stakeholders
3. Track progress via beads

---

*Generated by: cdocs/polecats/chrome polecat*
*For bead: cd-31v*
