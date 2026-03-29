# Architectural Drift Review — vo-cdpi

**Bead:** Store definition source in KV after lint  
**Date:** 2026-03-23  
**Reviewer:** Architectural Drift Agent  
**Status:** `STATUS: PERFECT`

---

## File Line Counts

| File | Lines | Limit | Status |
|------|-------|-------|--------|
| `crates/vo-api/src/handlers/definitions.rs` | 260 | 300 | ✅ |
| `crates/vo-api/src/types/requests.rs` | 49 | 300 | ✅ |

No files exceed the 300-line limit.

---

## 1. Module Cohesion

**PASS.** Both files have a single, clear responsibility:
- `definitions.rs` — sole handler for POST `/api/v1/definitions/:type` plus co-located unit tests. Standard Rust `#[cfg(test)] mod tests` pattern.
- `requests.rs` — lean DTO file containing only request structs. 49 lines, perfectly scoped.

---

## 2. Explicit State Transitions

**PASS.** The handler implements a clear, matchable state machine:

```
Validate workflow_type ──empty──> 400 BAD_REQUEST
        │
        ▼
Lint source ──parse error──> 400 BAD_REQUEST
        │
        ▼
Check severity ──errors──> 200 OK (diagnostics, no store)
        │
        ▼
    Store to KV ──success──> 200 OK (valid=true, diagnostics)
                  ──failure──> 500 INTERNAL_SERVER_ERROR
```

Each branch returns a distinct `(StatusCode, Json)` tuple. No hidden control flow.

---

## 3. Primitive Obsession — Observation (Pre-existing, not introduced by this bead)

The `DefinitionRequest` struct uses raw `String` for `workflow_type`:

```rust
pub struct DefinitionRequest {
    pub source: String,        // free-text source code — newtype adds no value
    pub workflow_type: String, // ⚠️ should be a domain newtype
}
```

The handler compensates with a runtime guard:
```rust
if req.workflow_type.trim().is_empty() { return 400; }
```

This is "validate, don't parse" — the opposite of Scott Wlaschin's "parse, don't validate" principle. The existing `newtypes.rs` already has `WorkflowName` (validates `[a-z][a-z0-9_]*`) and `SignalName` (validates `[a-z][a-z0-9_]+`). A `WorkflowType` newtype following the same pattern would make empty/whitespace workflow_types **unrepresentable** at the deserialization boundary, eliminating the need for the handler guard entirely.

**However:** This is a **pre-existing pattern** — `V3StartRequest` also uses raw `String` for `namespace`, `workflow_type`, and `paradigm`. This bead's scope is "store definition source in KV after lint," not refactoring request DTOs to newtypes. The bead actually *improved* the situation by adding the validation guard to prevent the malformed KV key (`default/`) that an empty workflow_type would produce.

**Recommendation:** File a future bead to introduce `WorkflowType` and `Namespace` newtypes in `newtypes.rs` and migrate `DefinitionRequest` and `V3StartRequest` to use them. This would eliminate the handler-level validation and make the illegal state unrepresentable.

---

## 4. DDD Compliance

| Principle | Status | Notes |
|-----------|--------|-------|
| Parse at boundaries | ⚠️ Partial | `workflow_type` validated at handler, not at deserialization. Pre-existing. |
| Make illegal states unrepresentable | ⚠️ Partial | Empty `workflow_type` can still be deserialized; caught at handler. Pre-existing. |
| Types as documentation | ✅ | `DefinitionRequest`, `DefinitionResponse`, `DiagnosticDto` clearly named |
| Single responsibility | ✅ | Each file has one job |

---

## 5. Test Quality

**PASS.** Tests follow the functional-core/imperative-shell pattern:
- Pure calculation tests (`definition_key` behavior) — no async, no HTTP.
- HTTP handler tests via `lint_only_app()` helper — tests parse-error, lint-error, and validation paths without requiring NATS.
- Invariant test (`definition_key_with_empty_workflow_type_produces_trailing_slash`) documents *why* the validation guard exists.

KV integration tests correctly deferred to E2E pipeline (require live NATS).

---

## Summary

No refactoring needed. The bead's code is clean, under 300 lines, with clear control flow and well-structured tests. The one DDD observation (primitive `String` for `workflow_type`) is pre-existing and out of scope. Recommended as a follow-up bead.

**STATUS: PERFECT**
