# Kani Justification

bead_id: wtf-j7wk
bead_title: "wtf-frontend: Simulate Mode Procedural — step through ctx calls, show checkpoint map"
phase: kani-justification
updated_at: 2026-03-21T18:05:00Z

## Formal Argument for Skipping Kani Model Checking

### State Machine Analysis

The `SimProceduralState` struct has three fields:
1. `checkpoint_map: HashMap<String, String>` - no invariants
2. `current_op: u32` - unsigned, bounded by operations count
3. `event_log: Vec<SimWorkflowEvent>` - no invariants

### State Transition Analysis

The only state transition is `provide_result()`:

```
Preconditions:
- result is non-empty
- total_ops > 0
- current_op < total_ops

Effect:
- checkpoint_map.insert(activity_id, result)
- event_log.push(ActivityCompleted)
- current_op += 1

Postconditions:
- current_op <= total_ops (guaranteed by precondition)
- checkpoint_map.len() == current_op (both updated atomically)
- event_log.len() == current_op (both updated atomically)
```

### Why Kani Is Not Required

1. **No complex state space**: The state machine is a simple counter (u32) with upper bound. There are no unreachable states or invalid state combinations.

2. **No memory safety issues**: The HashMap and Vec are standard library collections with well-understood semantics. No custom memory management.

3. **Compile-time guarantees**:
   - `u32` is unsigned, preventing negative values
   - `#[deny(clippy::panic)]` and `#[forbid(unsafe_code)]` enforce no panic paths

4. **Runtime guards**: The `provide_result()` function validates preconditions before any state mutation, returning errors instead of panicking.

5. **Trivial state transitions**: The only operation is incrementing a counter and appending to collections. The mathematical properties are:
   - `current_op` starts at 0
   - `current_op` increases by exactly 1 on each successful `provide_result()`
   - `current_op` never exceeds the `total_ops` parameter (enforced by precondition check)

### What Could Go Wrong Without Kani

**Scenario**: Overflow of `current_op`

**Analysis**: `current_op` is `u32` (max ~4 billion). In realistic UI usage, no workflow has billions of operations. Even if it did, the precondition check `current_op < total_ops` would block advancement before overflow.

**Formal guarantee**: `current_op` is bounded by `total_ops` which is `usize`. The conversion `u32::try_from(total_ops)` fails (returns error) if `total_ops > u32::MAX`, preventing the overflow.

### Conclusion

Kani model checking is not necessary for this implementation because:
- The state space is trivial (single counter)
- All state transitions are validated before execution
- No unsafe code exists
- No complex memory management

**Formal reasoning provided above is sufficient for this low-risk state machine.**

## Alternative: If Kani Were Run

If Kani were executed, it would verify:
1. No panics in `provide_result()` path
2. No buffer overflows in HashMap/Vec operations
3. Correctness of bounds checks

All of which are already guaranteed by Rust's type system and the standard library's memory safety.
