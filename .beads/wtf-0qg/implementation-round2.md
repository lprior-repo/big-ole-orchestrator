# Round 2 Implementation — Black-Hat Defect Fixes

## Fixes Applied

### N-07 (CRITICAL): `persist_metadata` failure now kills actor and returns error

**Files changed:**
- `crates/vo-actor/src/messages/errors.rs` — Added `PersistenceFailed(String)` variant to `StartError`
- `crates/vo-actor/src/master/handlers/start.rs` — `spawn_and_register` now calls `actor_ref.stop()` on persist failure and returns `Err(StartError::PersistenceFailed(...))` instead of logging and continuing

**Invariant enforced:** The caller is never told success when metadata is not persisted. A spawned-but-unregistered actor is immediately killed, preventing orphaned invisible instances.

### N-09 (HIGH): Heartbeat persistence failure now logged

**File changed:**
- `crates/vo-actor/src/instance/handlers.rs` — Replaced `let _ = store.put_heartbeat(...)` with `if let Err(e) = store.put_heartbeat(...) { tracing::error!(error = %e, "heartbeat persistence failed"); }`

**Rationale:** Silent swallow of heartbeat persistence errors masked infrastructure degradation. The error is now surfaced to operators via structured logging while the instance continues (heartbeat is best-effort, not critical path).

### N-06 (MEDIUM): DRY shared `InstanceArguments` construction

**Files changed:**
- `crates/vo-actor/src/messages/instance.rs` — Added `InstanceSeed` struct bundling per-instance identity fields (`namespace`, `instance_id`, `workflow_type`, `paradigm`, `input`)
- `crates/vo-actor/src/master/state.rs` — Added `OrchestratorState::build_instance_args(seed: InstanceSeed) -> InstanceArguments` as single source of truth for wiring config+registry fields
- `crates/vo-actor/src/master/handlers/start.rs` — Deleted `build_args()` (6 params), replaced with `InstanceSeed` construction + `state.build_instance_args(seed)` call
- `crates/vo-actor/src/master/handlers/heartbeat.rs` — `build_recovery_args()` now constructs `InstanceSeed` from `InstanceMetadata` and delegates to `state.build_instance_args()`

**Design rationale:** The two construction sites (fresh spawn and crash recovery) had identical 12-field struct literals. The shared constructor ensures that adding a new field to `InstanceArguments` cannot be missed in one path. `InstanceSeed` keeps the method at 2 params (under the 5-param limit).

## Constraint Adherence

| Constraint | Status |
|---|---|
| Zero `unwrap`/`expect` outside tests | Pass |
| Functions under 25 lines | Pass |
| Max 5 params per function | Pass |
| `cargo check -p vo-actor` | Pass (zero errors) |
| `cargo test -p vo-actor` | Pass (96/96 tests, 0 failures) |
| No regressions | Pass |

## Changed Files

1. `crates/vo-actor/src/messages/errors.rs` (+1 variant)
2. `crates/vo-actor/src/messages/instance.rs` (+8 lines — `InstanceSeed`)
3. `crates/vo-actor/src/master/state.rs` (+15 lines — `build_instance_args`)
4. `crates/vo-actor/src/master/handlers/start.rs` (rewrote `spawn_and_register`, deleted `build_args`, added seed construction)
5. `crates/vo-actor/src/master/handlers/heartbeat.rs` (rewrote `build_recovery_args`)
6. `crates/vo-actor/src/instance/handlers.rs` (added error logging to heartbeat)
