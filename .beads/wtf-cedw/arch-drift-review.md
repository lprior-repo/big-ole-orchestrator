# Arch Drift Review — wtf-cedw

**Date:** 2026-03-23
**Files reviewed:**
- `crates/wtf-actor/src/instance/handlers.rs` (263 lines)
- `crates/wtf-actor/src/instance/state.rs` (79 lines)

## Line Count

| File | Lines | Limit | Status |
|------|-------|-------|--------|
| handlers.rs | 263 | 300 | ✅ |
| state.rs | 79 | 300 | ✅ |

## DDD / Scott Wlaschin Compliance

- **Types as documentation** ✅ — `InstanceState` fields have clear doc comments; `InstancePhase`, `ParadigmState`, `ActivityId`, `TimerId` are proper domain types (not raw primitives).
- **No primitive obsession** ✅ — `ActivityId::new(...)`, `TimerId::new(...)` used consistently; signal names are the only `String` keys (acceptable — signal names are inherently dynamic identifiers).
- **Parse at boundaries** ✅ — `ActivityId` and `TimerId` are parsed from raw `&str` at the handler boundary.
- **Single responsibility** ✅ — `state.rs` owns the state struct + initialization; `handlers.rs` owns message dispatch + event injection.
- **Module cohesion** ✅ — procedural workflow handlers correctly delegated to `super::procedural` submodule.

## Verdict

**STATUS: PERFECT**
