# ADR 039 (v2): Hierarchical Lifecycle State Machine

## Status
Accepted

## Context
Flat lifecycle enums work for simple engines, but Veloxide now has:
1. exact-once scheduling,
2. managed effects,
3. compensation,
4. hibernation,
5. recovery,
6. unsafe fallback nodes.

Without a hierarchical model, lifecycle logic will drift into illegal mixed states and sprawling guard code.

State-machine libraries in the Rust ecosystem already show the right pattern. We should steal it directly.

## Decision
We model workflow lifecycle as a hierarchical state machine with typed transition boundaries.

### 1. Superstates
The top-level lifecycle contains at least:
1. `Active`
2. `Suspended`
3. `Recovering`
4. `Compensating`
5. `Terminal`

### 2. Example Substates
- `Active::Deciding`
- `Active::Scheduling`
- `Active::ExecutingPure`
- `Active::PreparingEffect`
- `Active::CommittingEffect`
- `Suspended::WaitingForTimer`
- `Suspended::WaitingForSignal`
- `Compensating::Planning`
- `Compensating::Executing`
- `Terminal::Completed`
- `Terminal::Failed`
- `Terminal::Cancelled`
- `Terminal::Compensated`

### 3. Typed Boundaries
The Engine should use typed boundary objects or equivalent compile-time discipline so that illegal states are unrepresentable where feasible.

Examples:
1. child spawn requires an owned current fence,
2. effect commit requires a previously prepared effect,
3. compensation execution requires a committed forward effect.

## Consequences
- **Positive:** The lifecycle model becomes easier to audit, test, and visualize.
- **Positive:** Illegal mixed states become much harder to express accidentally.
- **Positive:** The future UI can render meaningful superstate/substate views instead of one giant flat enum.
- **Negative:** The Engine state model becomes more formal and more verbose.
