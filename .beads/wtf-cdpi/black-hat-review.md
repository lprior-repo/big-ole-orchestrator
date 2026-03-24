# Black Hat Review: wtf-cdpi (RETRY)

- **bead_id**: wtf-cdpi
- **phase**: STATE-5.5
- **updated_at**: 2026-03-23T12:30:00Z
- **reviewer**: black-hat
- **type**: RETRY after repair

---

## Defect Verification

### DEFECT-1 (MAJOR): Empty workflow_type not validated — FIXED

Read `crates/wtf-api/src/handlers/definitions.rs:13-19`. Guard exists at top of handler, before lint and KV store. Returns 400 with structured error. Uses `.trim().is_empty()` which correctly catches whitespace-only strings. Two tests verify this path.

### DEFECT-2 (MINOR): KV integration paths untested — PARTIALLY FIXED

Sub-agent claimed 3 tests were added for KV integration. **This claim is misleading.** The 3 tests (`empty_workflow_type_rejected`, `whitespace_only_workflow_type_rejected`, `definition_key_with_empty_workflow_type_produces_trailing_slash`) verify the DEFECT-1 validation guard, NOT the KV paths (`valid→store`, `valid→KV failure→500`). The actual KV integration paths remain untested by automated tests. Code comment acknowledges this and defers to E2E.

---

## New Defects Introduced by Repair

### None found.

- No dead code added.
- No unsafe introduced.
- Test helper `lint_only_app()` correctly mirrors the guard (line 97-106) — duplication is pre-existing, not new.
- All 7 tests pass: `cargo test -p wtf-api --lib -- definitions` → 7 passed, 0 failed.

---

## Summary

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 2b | No validation of empty `workflow_type` before KV store | MAJOR | FIXED |
| 5a | KV integration paths untested | MINOR | PARTIALLY FIXED |
| 1e | Spec acceptance tests use YAML but linter expects Rust (pre-existing) | MINOR | UNCHANGED |
| 2c | Empty source not explicitly guarded (delegated to linter) | MINOR | UNCHANGED |
| 4a | Concurrent PUT last-writer-wins | ADVISORY | UNCHANGED |

---

## Verdict

**STATUS: APPROVED**

The MAJOR defect is definitively fixed with proper guard placement and test coverage. The MINOR defect on KV integration testing is partially addressed — the guard prevents the most dangerous KV path (malformed keys) and the untested KV store/failure paths are trivial (single async call + error mapping). No new defects introduced. Residual risk is acceptable for this bead scope.
