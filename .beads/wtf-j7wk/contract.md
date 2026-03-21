# Contract Specification

bead_id: wtf-j7wk
bead_title: "wtf-frontend: Simulate Mode Procedural — step through ctx calls, show checkpoint map"
phase: contract
updated_at: 2026-03-21T17:30:00Z

## Context

- **Feature**: SimulateProcedural UI component for stepwise workflow replay
- **Location**: `crates/wtf-frontend/src/ui/simulate_mode.rs`
- **Domain terms**:
  - `SimProceduralState` — local UI state for the simulator
  - `checkpoint_map` — accumulated key→value pairs from completed ops
  - `current_op` — index into the ordered ops list
  - `event_log` — Vec of WorkflowEvent appended as user advances
  - `CtxActivity`, `CtxSleep`, `CtxWaitSignal` — node types representing context operations
- **Assumptions**:
  - Workflow graph is available via existing `Workflow` struct
  - `WorkflowEvent` types from `wtf_common` are used for event_log
  - Dioxus 0.7 signals/stores for UI state management
- **Open questions**:
  - How to extract ctx nodes from an existing workflow graph? (pending parent bead wtf-7n80)
  - Mock result input format (string vs JSON)?

## Preconditions

1. **P1**: `checkpoint_map` must be initialized as empty `HashMap`
2. **P2**: `current_op` must be `0` initially (no operations started)
3. **P3**: `event_log` must be empty initially
4. **P4**: `current_op` must never exceed the length of the ops list
5. **P5**: Result input must be non-empty before "Complete" is actionable

## Postconditions

1. **Q1**: After "Complete" clicked with valid result:
   - A synthetic `WorkflowEvent::ActivityCompleted { activity_id, result, duration_ms: 0 }` is appended to `event_log`
   - A new entry is added to `checkpoint_map` (key = activity_id, value = result)
   - `current_op` is incremented by 1
2. **Q2**: After advancing past all ops:
   - `current_op` equals the total number of ops (terminal state)
   - No further "Complete" actions are possible
3. **Q3**: `checkpoint_map` always reflects exactly the completed operations in order
4. **Q4**: `event_log` is append-only (no removals or modifications)

## Invariants

1. **I1**: `0 <= current_op <= ops.len()`
2. **I2**: `checkpoint_map.len() == current_op` (one checkpoint per completed op)
3. **I3**: `event_log.len() == current_op` (one event per completed op)

## Error Taxonomy

- `Error::EmptyResult` — when user clicks "Complete" with empty result input
- `Error::AlreadyCompleted` — when attempting to advance past terminal state
- `Error::NoOpsAvailable` — when workflow has zero ctx operations to simulate

## Contract Signatures

```rust
// State definition (UI layer, not persisted)
struct SimProceduralState {
    checkpoint_map: HashMap<String, String>,
    current_op: u32,
    event_log: Vec<WorkflowEvent>,
}

impl SimProceduralState {
    fn new() -> Self;
    fn provide_result(&mut self, result: String, activity_id: &str) -> Result<(), Error>;
    fn can_advance(&self, total_ops: usize) -> bool;
    fn current_op_index(&self) -> u32;
}
```

## Type Encoding

| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| P1: checkpoint_map initialized | Compile-time | `HashMap::new()` default |
| P2: current_op = 0 initially | Compile-time | `u32::default()` = 0 |
| P3: event_log empty initially | Compile-time | `Vec::new()` default |
| P4: current_op <= ops.len() | Runtime-checked | `can_advance()` guard |
| P5: result non-empty | Runtime-checked constructor | `NonEmptyString::try_from()` |

## Violation Examples

- **VIOLATES P5**: `provide_result("")` → returns `Err(Error::EmptyResult)`
- **VIOLATES P4**: `provide_result("ok", "act-1")` when `current_op == ops.len()` → returns `Err(Error::AlreadyCompleted)`

## Ownership Contracts

- `checkpoint_map`: Owned `HashMap`, mutated on each `provide_result` call
- `current_op`: Owned `u32`, incremented monotonically
- `event_log`: Owned `Vec`, appended only via `provide_result`

## Non-goals

- Network communication or real activity dispatch
- Persistence of simulator state
- Multi-user or concurrent simulation sessions
