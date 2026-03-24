# Architectural Drift Review — wtf-bx19

**Date:** 2026-03-23
**Agent:** architectural-drift

## Line Counts

| File | Lines | Limit | Status |
|------|-------|-------|--------|
| `crates/wtf-actor/src/dag/parse.rs` | 240 | 300 | ✅ Pass |
| `crates/wtf-actor/src/dag/state.rs` | 66 | 300 | ✅ Pass |
| `crates/wtf-actor/src/instance/state.rs` | 79 | 300 | ✅ Pass |

## DDD / Scott Wlaschin Compliance

### `dag/parse.rs`
- **Parse, don't validate**: Returns `Result<HashMap<NodeId, DagNode>, DagParseError>` — unvalidated JSON is rejected at the boundary; internal types are guaranteed correct.
- **NewTypes over primitives**: `NodeId` wraps `String`; predecessors use `Vec<NodeId>`.
- **Structured errors**: `DagParseError` variants carry domain context (`node_id`, `predecessor_id`, `index`, `field`) — not raw strings.
- **Pure functions**: Every function in this file is a pure calculation (no I/O, no async). The one bounded mutation (Kahn's algorithm) is isolated in `kahn_run` and explicitly documented.
- **Single responsibility**: Parse JSON → extract nodes → validate predecessors → detect cycles.

### `dag/state.rs`
- **Domain types**: `NodeId` NewType with `Display`, `From<&ActivityId>`, `new()`, `as_str()` — proper encapsulation.
- **State as documentation**: `DagActorState` fields clearly express the domain: `completed`, `in_flight`, `failed`, `applied_seq`.
- **Single responsibility**: Only type definitions and a constructor. No logic leaks in.

### `instance/state.rs`
- **Domain types used**: `ActivityId`, `TimerId`, `RpcReplyPort<Result<Bytes, WtfError>>` — no raw primitives.
- **Factory pattern**: `initial()` constructs from `InstanceArguments`; `initialize_paradigm_state()` dispatches cleanly via match.
- **Single responsibility**: Instance state definition + paradigm initialization. 79 lines total.

## Refactoring Required

None.

## Verdict

**STATUS: PERFECT**
