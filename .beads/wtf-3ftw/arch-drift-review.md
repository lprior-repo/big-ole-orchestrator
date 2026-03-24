# Architectural Drift Review — wtf-3ftw

**File under review:** `crates/wtf-actor/src/fsm/definition.rs`

## Line Counts

| File | Lines | Status |
|------|-------|--------|
| `definition.rs` | **209** | ✅ Under 300 |
| `definition_tests.rs` | **235** | ✅ Under 300 |
| `handlers.rs` | **83** | ✅ Under 300 |
| `state.rs` | **37** | ✅ Under 300 |
| `types.rs` | **39** | ✅ Under 300 |
| `tests.rs` | **161** | ✅ Under 300 |

## DDD Compliance

| Principle | Assessment |
|-----------|-----------|
| Parse at boundaries | ✅ Private serde intermediaries (`FsmGraph`, `FsmTransitionJson`, `FsmEffectJson`) are wire-format-only; public type is `FsmDefinition` |
| Illegal states unrepresentable | ✅ `FsmDefinition` is only constructible via `parse_fsm` (validates all fields) or `new()` + builder methods — invalid JSON cannot produce an instance |
| Single responsibility | ✅ `definition.rs` owns parsing + definition; `state.rs` owns actor state; `types.rs` owns result enums; `handlers.rs` owns message dispatch |
| Primitive obsession | ⚠️ Minor — state names and events are raw `String`. Acceptable here because they are unvalidated identifiers from JSON with no domain invariants beyond non-emptiness (already enforced by `Option` → `ok_or`) |
| Pure functions, no I/O | ✅ `parse_fsm` is fully pure — zero logging, zero I/O |

## Verdict

**STATUS: PERFECT**

No refactoring needed. The FSM module demonstrates good architectural hygiene:
proper type boundaries between wire and domain, tests extracted to a dedicated file,
and every file stays well under the 300-line ceiling.
