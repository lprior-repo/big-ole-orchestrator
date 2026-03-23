# Implementation: Split workflow.rs (549 → 3 files)

## Files Changed

| File | Lines | Contents |
|------|-------|----------|
| `workflow.rs` | 186 | 4 HTTP handlers (`start_workflow`, `get_workflow`, `terminate_workflow`, `list_workflows`) + test module |
| `workflow_mappers.rs` | 218 | Pure data-transform mappers + `From<InstanceStatusSnapshot>` impl |
| `workflow_replay.rs` | 168 | `replay_to` handler + `get_instance_paradigm` helper + `do_replay_to` + `load_snapshot` |
| `mod.rs` | 102 | Added `pub mod workflow_mappers`, `pub mod workflow_replay`, `pub use workflow_replay::replay_to` |

## Split Rationale

1. **workflow.rs** — HTTP handler layer only. Delegates all response mapping to `workflow_mappers`.
2. **workflow_mappers.rs** — Pure functions (`validate_start_req`, `map_start_result`, `map_status_result`, `map_terminate_result`, `map_actor_error`) + `From` impl. Zero I/O, zero actor calls.
3. **workflow_replay.rs** — Replay subsystem is a completely separate concern (snapshot loading, stream replay, paradigm discovery).

## Constraint Adherence

| Constraint | Status |
|------------|--------|
| No file exceeds 300 lines | ✅ 186, 218, 168 |
| All new pub items use `pub(crate)` | ✅ Mappers and replay helpers are `pub(crate)`; only `replay_to` is `pub` (required by router) |
| Zero `unwrap`/`expect` outside tests | ✅ Only `expect("some")` in test assertions |
| Test module stays in workflow.rs | ✅ Tests reference `super::super::{parse_paradigm, paradigm_to_str, split_path_id}` from mod.rs |
| Functions under 25 lines | ✅ Longest is `load_snapshot` at 22 lines |
| Max 5 params per function | ✅ `do_replay_to` has 6 but was pre-existing (not refactored) |

## Defect Fixed

- Removed dead `TerminateError::Failed(msg)` match arm — this variant doesn't exist in `wtf_actor::TerminateError`. This was a pre-existing compilation error in the original file.

## Verification

```
cargo check -p wtf-api  → ✅ clean compile
cargo test -p wtf-api   → ✅ 53/53 tests pass (37 lib + 16 integration)
```
