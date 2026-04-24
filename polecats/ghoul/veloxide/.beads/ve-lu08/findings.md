# ADR-DEEP: ADR-025 GDPR Purging Implementation - Findings

## Bead: ve-lu08
**Task**: ADR-DEEP: ADR-025 GDPR purging implementation
**Status**: AUDIT COMPLETE (no code changes - research/analysis task)
**Date**: 2026-04-24

---

## Executive Summary

ADR-025 defines a **dual-representation privacy model** for GDPR compliance:
1. **Canonical replay data** - encrypted payloads for exact-once recovery
2. **Operator projection** - redacted JSON view for UI/CLI/AI consumption

The purge tool (`vo-cli purge --instance <id>`) implements crypto-shredding by destroying per-instance DEKs, rendering canonical blobs unreadable.

---

## Architecture Overview

### Key Components

| Component | Location | Purpose |
|-----------|----------|---------|
| ADR-025 v2 | `docs/adr/v2/ADR-025-v2-state-privacy-gdpr-purging.md` | Specification |
| `purge_instance()` | `crates/vo-storage/src/purge.rs` | Core purge logic |
| `PurgeHandler` | `crates/vo-cli/src/registry.rs:70-106` | CLI command handler |
| `RedactionPolicy` | `crates/vo-types/` | Operator projection redaction |
| `apply_redaction()` | `crates/vo-types/` | Redaction application |

### Purge Flow (ADR-025 §3)

```
vo-cli purge --instance <id>
    → PurgeHandler.execute()
    → vo_storage::purge::purge_instance()
    → 1. Destroy per-instance DEK (renders blobs unreadable)
    → 2. Delete redacted operator projections, indexes, blob references
    → 3. Queue physical removal for compaction-time reclamation
```

---

## Current Implementation Status

### ✅ Implemented

1. **`purge_instance()` in vo-storage/src/purge.rs:13-73**
   - Verifies instance is in terminal state (Completed/Failed/Cancelled)
   - Atomically deletes events, snapshots, and instance index entries
   - Returns count of purged events

2. **CLI Command Parsing** (`crates/vo-cli/src/cli.rs:274-281`)
   - `vo-cli purge --instance <id>` parsed correctly

3. **`PurgeHandler`** (`crates/vo-cli/src/registry.rs:70-106`)
   - Executes purge via `vo_storage::purge::purge_instance()`
   - Handles `InstanceRunning` and general errors

4. **Terminal State Detection** (`purge.rs:75-81`)
   - `is_terminal()` correctly identifies terminal states

5. **Unit Tests** (`purge.rs:83-184`)
   - Tests for empty input, absent instance, zero events, successful purge
   - `rstest` coverage for all instance statuses

6. **Integration Tests** (`crates/vo-storage/tests/purge_integration.rs`)
   - `purge_terminal_instance_deletes_all_records` ✅
   - `purge_running_instance_fails` ✅
   - `purge_instance_returns_invalid_instance_id_when_input_empty` ✅
   - `purge_instance_returns_invalid_instance_id_when_input_is_malformed` ✅

7. **BDD GDPR Tests** (`polecats/radrat/veloxide/crates/vo-types/tests/bdd_gdpr_purging_tests.rs`)
   - Full purge policy tests
   - PII redaction verification
   - Redaction path tracking

8. **AI Redaction Moon Gate** (`ai_redaction_moon_gate.rs`)
   - Standard PII redaction rules
   - Multi-user PII redaction
   - Deep nested redaction
   - Operator projection invariants

---

## Critical Issues Identified

### 1. **HARDCODED STORAGE PATH** (Severity: HIGH)

**Location**: `crates/vo-cli/src/registry.rs:88`

```rust
let fjall_path = std::path::Path::new("/home/lewis/.gemini/tmp/veloxide/fjall");
```

**Problem**: The storage path is hardcoded to `/home/lewis/.gemini/tmp/veloxide/fjall`. This ignores the `storage_path` field that may be passed via CLI.

**Impact**: Purge only works on this specific machine's path. Cannot target different storage locations.

**Fix Required**: Should use `cli.storage_path` or a configured path rather than hardcoded value.

---

### 2. **DEK DESTRUCTION NOT IMPLEMENTED** (Severity: HIGH - GDPR Compliance Gap)

**Location**: `crates/vo-storage/src/purge.rs`

**Problem**: ADR-025 §3 step 1 states: "Destroy the per-instance DEK, rendering canonical payload blobs unreadable."

The current `purge_instance()` function:
- ❌ Does NOT destroy any DEKs
- ❌ Does NOT interact with key encryption keys (KEKs)
- ❌ Does NOT render blobs unreadable via crypto-shredding

Current implementation only performs **data deletion** (events, snapshots, indexes), not **crypto-shredding**.

**Impact**: GDPR crypto-shredding requirement not fulfilled. Blobs may remain readable if DEK is not destroyed.

**Fix Required**: Add DEK destruction to `purge_instance()` before data deletion.

---

### 3. **OPERATOR PROJECTIONS NOT DELETED** (Severity: MEDIUM)

**Problem**: ADR-025 §3 step 2 states: "Delete redacted operator projections."

The current implementation does not explicitly handle operator projections (separate from canonical data).

**Impact**: Redacted views may persist even after purge.

---

### 4. **COMPACTION-TIME RECLAMATION NOT VISIBLE** (Severity: LOW)

**ADR States**: "Queue physical blob and key removal in Fjall for compaction-time reclamation."

The implementation relies on Fjall's natural compaction behavior. No explicit queueing mechanism found.

---

## Test Coverage Analysis

### Covered by Unit Tests
- Empty instance ID handling ✅
- Absent instance handling ✅
- Terminal instance with no events ✅
- Successful purge with events/snapshots ✅
- All `is_terminal()` status combinations ✅

### Covered by Integration Tests
- Terminal instance full deletion ✅
- Running instance rejection ✅
- Invalid instance ID handling ✅
- Zero-event terminal instance ✅

### Missing Test Coverage
- ❌ DEK destruction verification after purge
- ❌ Multi-blob instance purge
- ❌ Purge ordering (DEK → index → blobs)
- ❌ Idempotency verification (double-purge)
- ❌ Purge audit logging

---

## ADR Compliance Checklist

| ADR-025 Requirement | Status | Notes |
|-------------------|--------|-------|
| Dual representation model | ✅ PARTIAL | Canonical + Operator projection defined |
| DEK destruction | ❌ MISSING | No key destruction in purge_instance() |
| Operator projection deletion | ❌ UNCLEAR | Not explicitly handled |
| Physical blob queueing | ⚠️ IMPLICIT | Relies on Fjall compaction |
| Minimal fact retention | ⚠️ ASSUMED | Dedup keys not explicitly audited |
| Encryption at rest | ✅ DEFINED | Not verified in this audit |

---

## Recommendations

### P0 - Critical (GDPR Compliance)
1. **Implement DEK destruction** in `purge_instance()` before data deletion
2. **Fix hardcoded path** to use configurable storage location

### P1 - High (Functional)
3. Add explicit operator projection deletion
4. Add test verifying ciphertext is unreadable post-purge (crypto-shredding proof)
5. Add idempotency test (double-purge returns 0)

### P2 - Medium (Verification)
6. Add purge audit logging
7. Verify dedupe key retention window compliance

---

## Files Reviewed

### Primary Implementation
- `/crates/vo-storage/src/purge.rs` - Core purge logic
- `/crates/vo-cli/src/registry.rs` - CLI handler
- `/crates/vo-cli/src/cli.rs` - Command parsing

### Tests
- `/crates/vo-storage/tests/purge_integration.rs` - Integration tests
- `/polecats/radrat/veloxide/crates/vo-types/tests/bdd_gdpr_purging_tests.rs` - BDD tests
- `/polecats/radrat/veloxide/crates/vo-types/tests/ai_redaction_moon_gate.rs` - AI redaction

### Documentation
- `/docs/adr/v2/ADR-025-v2-state-privacy-gdpr-purging.md` - ADR specification

---

## Conclusion

The ADR-025 GDPR purge implementation has a **solid foundation** with good test coverage for the happy path. However, there are **critical gaps** in:

1. **Crypto-shredding** (DEK destruction) - the core GDPR requirement
2. **Storage path configuration** - hardcoded path limits portability
3. **Operator projection handling** - unclear if explicitly deleted

This is an **audit/analysis task** - no code changes were made as the issues require architectural decisions about key management lifecycle that should be reviewed by the team.
