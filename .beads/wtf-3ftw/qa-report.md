# QA Report: vo-3ftw — parse_fsm

**Date**: 2026-03-23
**File**: `crates/vo-actor/src/fsm/definition.rs` (405 lines)

---

## 1. QA CHECKLIST

### 1.1 parse_fsm exists
PASS — `parse_fsm(graph_raw: &str) -> Result<FsmDefinition, ParseFsmError>` at line 103.
Signature is pure function, zero I/O, zero logging. Correct.

### 1.2 unwrap/expect in production code
PASS — Zero `unwrap()` or `expect()` calls in production code (lines 1–195).
All 18 `unwrap`/`expect` occurrences are inside `#[cfg(test)]` block (lines 196–405).

### 1.3 Test execution
```
cargo test -p vo-actor --lib -- fsm::definition
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 106 filtered out
```
PASS — All 17 tests green.

### 1.4 Line count
405 lines (including 210 lines of tests). Production code: ~195 lines.
FAIL — Exceeds 300-line limit. (Note: if tests are excluded, it's under 300. But the file as a whole is 405.)

---

## 2. RED QUEEN ATTACKS

### 2.1 Empty JSON object `{}`
**PASS** — Returns `Err(ParseFsmError::MissingField("transitions"))`. Handled by explicit check at line 113.

### 2.2 Missing transitions key
**PASS** — Same as 2.1. `raw.get("transitions").is_none_or(|v| !v.is_array())` catches both missing and non-array.

### 2.3 Transition with missing "to" field
**PASS** — Returns `Err(ParseFsmError::MissingField("to"))` via `validate_transition` at line 67. Tested by `ep3_transition_missing_from` (and the same path covers missing "to").

### 2.4 Non-UTF8 in payload
**N/A** — Input is `&str` (already valid UTF-8). Payload is extracted from JSON string values, which serde guarantees are UTF-8. The `Bytes::from(s.as_bytes().to_vec())` conversion is safe. No attack surface.

### 2.5 Clippy unwrap_used
**PASS** — `cargo clippy -p vo-actor -- -W clippy::unwrap_used` produces zero warnings for `definition.rs`.

---

## 3. BLACK HAT REVIEW

### 3.1 Signature matches usage
PASS — `parse_fsm` is re-exported via `crates/vo-actor/src/fsm.rs:8`:
```rust
pub use definition::{parse_fsm, FsmDefinition, ParseFsmError};
```
Signature `parse_fsm(graph_raw: &str) -> Result<FsmDefinition, ParseFsmError>` matches all call sites.

### 3.2 Hallucinated API calls
PASS — No hallucinated API calls. All used types are:
- `serde_json` (standard serde JSON)
- `bytes::Bytes` (crate dependency)
- `vo_common::EffectDeclaration` (workspace crate)
- Standard library (`HashMap`, `HashSet`)

---

## 4. CONTRACT DEVIATIONS

| Contract Item | Status | Notes |
|---|---|---|
| Pure function signature | PASS | Matches exactly |
| transitions array | PASS | Required, validated |
| Optional terminal_states | PASS | `Option<Vec<String>>`, defaults to empty |
| Optional initial_state | **FAIL** | Not implemented. No `initial_state` field in `FsmGraph` or `FsmDefinition`. |
| Effects with effect_type + payload | PASS | `FsmEffectJson` handles both |
| Error enum: InvalidJson, MissingField, InvalidEffect | PASS | All three variants present |
| 17 tests (5 happy, 4 error, 8 edge, 1 E2E) | PASS | 5 + 4 + 7 + 1 = 17 |
| File < 300 lines | **WARN** | 405 lines total; ~195 production lines |

---

## 5. VERDICT

**REJECTED**

Two issues require resolution:

1. **Missing `initial_state`** — Contract specifies optional `initial_state` field. Not present in `FsmGraph`, not parsed, not stored in `FsmDefinition`. Either implement it or formally amend the contract.

2. **File exceeds 300-line limit** — 405 lines. Must be split (e.g., move tests to `definition_tests.rs` or extract `FsmDefinition` impls to separate file).
