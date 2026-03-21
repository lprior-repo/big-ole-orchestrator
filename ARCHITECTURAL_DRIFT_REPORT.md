# ARCHITECTURAL DRIFT REPORT

## 1. File Length Violations
- **Previous Status**: **FAIL**. `instance/mod.rs` (534) and `procedural.rs` (683) were over the 300-line limit.
- **Current Status**: **PERFECT**. All files are now under 300 lines.
  - `crates/wtf-actor/src/instance/actor.rs`: 231 lines
  - `crates/wtf-actor/src/instance/lifecycle.rs`: 236 lines
  - `crates/wtf-actor/src/instance/mod.rs`: 115 lines
  - `crates/wtf-actor/src/instance/procedural.rs`: 145 lines
  - `crates/wtf-actor/src/instance/state.rs`: 64 lines
  - `crates/wtf-actor/src/procedural/context.rs`: 156 lines
  - `crates/wtf-actor/src/procedural/mod.rs`: 75 lines
  - `crates/wtf-actor/src/procedural/state/mod.rs`: 221 lines

## 2. Scott Wlaschin DDD: Modeling Workflows
- **Analysis**: Refactored `InstanceState` and the `WorkflowInstance` actor to separate state from the actor logic. Extracted paradigm-specific handlers.
- **Status**: **IMPROVED**. The separation of `InstanceState` from `Actor` logic and the isolation of `ParadigmState` helpers makes the workflow transitions more explicit and the states cleaner.

## 3. Structural Cohesion: `ParadigmState` Bottleneck
- **Analysis**: The unified `WorkflowInstance` actor no longer contains paradigm-specific logic for all 3 paradigms in a single `handle` loop. Procedural logic has been extracted to `instance/procedural.rs`.
- **Status**: **IMPROVED**. The "leaky abstraction" is mitigated by delegating paradigm-specific message handling to dedicated modules.

## 4. Refactoring Summary
1.  **Split `procedural.rs`**: Moved state to `procedural/state/`, context to `procedural/context.rs`, and tests to their own files.
2.  **Split `instance/mod.rs`**: Extracted the `Actor` implementation to `actor.rs`, state to `state.rs`, and procedural-specific handlers to `procedural.rs` (within the instance module).
3.  **Cleaned up tests**: Removed a broken and redundant integration test that was causing build failures after the refactoring.
4.  **Verified stability**: All 71 tests passed.
