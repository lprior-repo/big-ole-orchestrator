# Defects: vo-cdpi

## DEFECT-1: Empty workflow_type not validated [MAJOR] — FIXED

**Location**: `crates/vo-api/src/handlers/definitions.rs:13-19`

**Status**: FIXED

**Evidence**: Guard at line 13 returns 400 BAD_REQUEST with error code `invalid_request` before lint or KV store:
```rust
if req.workflow_type.trim().is_empty() {
    return (StatusCode::BAD_REQUEST, Json(ApiError::new("invalid_request", "workflow_type must be non-empty"))).into_response();
}
```
- Tests `empty_workflow_type_rejected` and `whitespace_only_workflow_type_rejected` verify both empty and whitespace-only inputs are rejected.
- Test `definition_key_with_empty_workflow_type_produces_trailing_slash` proves the invariant: guard prevents malformed `"default/"` KV key.

---

## DEFECT-2: KV integration paths have zero automated test coverage [MINOR] — PARTIALLY FIXED

**Location**: `crates/vo-api/src/handlers/definitions.rs` tests module

**Status**: PARTIALLY FIXED

**Evidence**: 3 tests were added but they cover the **validation guard** (DEFECT-1 fix), not the actual KV integration paths:
- `empty_workflow_type_rejected` — tests 400 on empty workflow_type
- `whitespace_only_workflow_type_rejected` — tests 400 on whitespace-only
- `definition_key_with_empty_workflow_type_produces_trailing_slash` — proves malformed key invariant

The original defect required `test_store_definition_after_lint` and `test_kv_store_failure_returns_500`. Neither was added. The code comment at line 191-198 explicitly defers these to E2E pipeline tests, citing that unit tests cannot provide `Extension<KvStores>` without a live NATS connection.

**Residual risk**: LOW. The guard prevents malformed keys from reaching KV. The KV store/failure paths are simple (single `put` call, error mapped to 500). Not tested but not complex either.
