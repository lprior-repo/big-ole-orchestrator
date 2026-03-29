# Contract Specification: capacity_check method (vo-47n)

## Context

- **Feature:** `capacity_check` method on `MasterOrchestrator`
- **Bead:** vo-47n
- **Location:** `vo-actor/src/master/`
- **Purpose:** Determines if the orchestrator can accept new workflow instances

## Domain Terms

| Term | Definition |
|------|------------|
| `running_count` | Number of currently active workflow instances in `OrchestratorState` |
| `max_concurrent` | Upper bound on concurrent workflow instances (configuration limit) |
| `capacity` | Ability to accept new workflow instances; available when `running_count < max_concurrent` |

## Vocabulary Drift Note

> **NOTE:** The bead specification uses `state.running_count` and `self.max_concurrent`. The current implementation uses `state.active.len()` and `state.config.max_instances`. The contract below follows the **bead specification** vocabulary.
> - `state.running_count` maps to `state.active.len()`
> - `self.max_concurrent` maps to `state.config.max_instances`

## Public API Signature

```rust
fn capacity_check(&self, state: &OrchestratorState) -> bool
```

### Signature Details

- **Receiver:** `&self` (immutable borrow of `MasterOrchestrator`)
- **Argument:** `state: &OrchestratorState` — reference to current orchestrator state
- **Returns:** `bool`
  - `true` → capacity available, new workflow may be spawned
  - `false` → at capacity limit, new workflow must be rejected

### Related Methods (for context)

```rust
// Existing implementation (state struct)
impl OrchestratorState {
    pub fn has_capacity(&self) -> bool {
        self.active.len() < self.config.max_instances
    }
}
```

## Preconditions

1. `state` must be a valid reference to an initialized `OrchestratorState`
2. `self.max_concurrent` must be initialized to a non-zero positive value (enforced at `MasterOrchestrator` construction)

## Postconditions

1. Returns `true` **iff** `state.running_count < self.max_concurrent`
2. Returns `false` **iff** `state.running_count >= self.max_concurrent`
3. The method is **pure** — no state mutation occurs

## Invariants

1. `running_count >= 0` (non-negative count)
2. `max_concurrent > 0` (limit must be positive, enforced at construction)
3. `running_count <= max_concurrent` (count never exceeds limit in valid state)

## Behavior Specification

### Critical Threshold

> **The boundary `running_count == max_concurrent` is the critical threshold where `capacity_check` transitions from `true` to `false`.** When `running_count` equals `max_concurrent`, the method returns `false`, indicating zero capacity remaining.

### Logic

```rust
fn capacity_check(&self, state: &OrchestratorState) -> bool {
    state.running_count < self.max_concurrent
}
```

### Truth Table

| `running_count` | `max_concurrent` | `capacity_check` result |
|-----------------|------------------|-------------------------|
| 0               | 3                | `true`                  |
| 2               | 3                | `true`                  |
| 3               | 3                | `false` (at boundary)   |
| 5               | 3                | `false`                 |

## Error Taxonomy

This method is **infallible** — it returns a boolean with no error conditions.

If capacity check is needed but state is unavailable, use `has_capacity()` on `OrchestratorState` as a fallback.

## Usage Context

This method is called **before** spawning a new workflow instance:

```rust
// Pseudocode
if orchestrator.capacity_check(&state) {
    // spawn new workflow
} else {
    // return AtCapacity error
}
```

## Acceptance Criteria

- [ ] Returns `true` when `running_count < max_concurrent`
- [ ] Returns `false` when `running_count >= max_concurrent`
- [ ] No state mutation occurs during check
- [ ] Method is `#[must_use]`

## Non-goals

- State mutation (use `register`/`deregister` for that)
- Error handling (infallible boolean return)
- Concurrency safety (assumes caller holds appropriate locks)

(End of file - total 123 lines)
