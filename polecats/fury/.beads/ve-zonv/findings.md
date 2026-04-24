# ADR-035 Upcasting Test Suite - Findings

## Bead: ve-zonv

## Status: QA/AUDIT COMPLETE

## Summary

Extensive ADR-035 upcasting test infrastructure already exists across the veloxide codebase. This bead was an audit task to evaluate coverage.

---

## Existing Test Infrastructure

### Test Files Analyzed (from radrat worktree):

1. **`crates/vo-core/src/upcaster/mod.rs`** (25 lines)
   - Defines `Upcaster` trait (re-exported from vo-types)
   - Defines `UpcasterRegistry` trait
   - Error types re-exported

2. **`crates/vo-core/src/upcaster/registry.rs`** (211 lines)
   - `UpcasterRegistryImpl` - concrete production implementation
   - `DefaultUpcasterRegistryBuilder`
   - Chain building and application logic

3. **`crates/vo-core/src/upcaster/error.rs`** (34 lines)
   - `UpcasterError` enum: NoUpcasterRegistered, DuplicateRegistration, UpcastingFailed, InvalidTargetVersion, CircularChain, InvalidUpcastedEnvelope

4. **`crates/vo-core/src/upcaster/error_tests.rs`** (172 lines)
   - Unit tests for UpcasterError variants
   - Equality, message format, Debug format tests

5. **`crates/vo-core/src/upcaster/event_envelope_error_tests.rs`** (229 lines)
   - Unit tests for EventEnvelopeError variants
   - All variants covered with equality and message tests

6. **`crates/vo-core/tests/upcaster_integration.rs`** (950 lines)
   - Comprehensive integration tests
   - Upcaster trait tests (determinism, idempotency, error paths)
   - UpcasterRegistry trait tests (registration, upcast_envelope, chain building)
   - Full workflow tests

7. **`crates/vo-core/src/replay/upcaster_tests.rs`** (169 lines)
   - Tests for `ReplayEngine::replay_with_upcaster`
   - Empty event list, v0 events upcast, full lifecycle, mixed versions
   - Upcasting failure propagation

8. **`crates/vo-core/tests/upcaster_proptest.rs`** (48 lines)
   - Proptest invariant: upcaster determinism

9. **`crates/vo-core/src/upcaster/kani_harnesses.rs`** (231 lines)
   - Formal verification harnesses for Kani
   - Version bound preservation proofs

---

## Test Coverage vs UC Test Cases

From `event-sourcing-projection-engine.md`:

| Test Case | Description | Status |
|-----------|-------------|--------|
| UC-001 | Events at current version pass through | ✅ Covered |
| UC-002 | Events at older version are upcast | ✅ Covered |
| UC-003 | Mixed-version events all upcast | ✅ Covered |
| UC-004 | Upcaster chain failure halts replay | ✅ Covered |
| UC-005 | No upcaster registered returns error | ✅ Covered |
| UC-006 | Upcasting chain exhaustion returns error | ⚠️ Partial (NoUpcasterRegistered covers this) |
| UC-007 | Upcasted events = native events | ✅ Covered |

---

## Gaps Identified

### 1. Multi-hop Upcaster Chain Testing
Current tests only cover v0 → v1 single-hop. With `MAX_SUPPORTED_VERSION = 1`, multi-hop chains (v0 → v1 → v2) cannot be tested. However, the registry code itself supports multi-hop chains correctly.

### 2. Kani Harness Interface Mismatch
In `kani_harnesses.rs:48`, `MockUpcaster::upcast` takes `&[u8]` but the actual `Upcaster` trait takes `&serde_json::Value`. This is a **critical bug** - the Kani proofs are testing the wrong interface.

### 3. Snapshot Upcasting Not Explicitly Tested
ADR-035 mentions snapshots carry their own schema version and may need upcasting. Snapshot-specific upcaster tests were not found in the examined files.

### 4. Lock Poison Edge Case
The `lock().map_err(|_| UpcasterError::UpcastingFailed("lock poisoned".to_string()))` path is exercised but not explicitly tested with a mock that causes panics.

---

## Findings Summary

**Positive:**
- Extensive test coverage exists for the upcaster system
- Unit, integration, replay, proptest, and formal verification layers all present
- Error variants comprehensively tested
- Happy path and error paths both covered

**Issues:**
- Kani harness has interface mismatch (tests wrong trait method signature)
- No explicit multi-hop chain tests (architectural limitation due to MAX_VERSION=1)
- Snapshot upcasting not explicitly tested

---

## Recommendation

1. **Fix the Kani harness** - the `MockUpcaster::upcast` must take `&serde_json::Value` to match the actual `Upcaster` trait
2. **Consider adding multi-hop tests** if MAX_SUPPORTED_VERSION increases
3. **Add snapshot upcasting tests** if snapshot upcasting is implemented per ADR-035

---

## Code Location Reference

- **Upcaster trait**: `vo_types::events::upcaster::Upcaster`
- **UpcasterRegistry trait**: `vo_core::upcaster::UpcasterRegistry`
- **Implementation**: `vo_core::upcaster::UpcasterRegistryImpl`
- **Tests**: `crates/vo-core/src/upcaster/` and `crates/vo-core/tests/`
