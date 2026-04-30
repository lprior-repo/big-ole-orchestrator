# ADR 048 (v2): Child Workflow Custody and Parent-Close Semantics

## Status
Accepted

## Context
The hierarchical lifecycle (ADR-039) establishes that workflows exist in parent-child relationships with typed boundary discipline. When a parent workflow reaches a terminal state, its children must not be left in undefined or unmanaged states.

Currently, the system has:
- `ParentChildRegistry` tracking child instances per parent actor
- `ShutdownPropagator` with configurable timeouts for graceful child shutdown
- A hierarchical lifecycle with terminal superstate `Terminal::Completed` and `Terminal::Failed`

**What is missing:** An explicit, documented policy for what happens to children when a parent closes. Without such a policy:

1. Children left in `Running` state after parent completion are effectively orphaned — no supervisor to monitor them, no lifecycle to govern them.
2. Children left in `Running` state after parent failure have no failure cascade — the failure signal stops at the parent.
3. Resource leaks accumulate because the engine has no rule for sweeping or reassigning abandoned children.
4. The typed boundary discipline from ADR-039 (§3) is incomplete: "child spawn requires an owned current fence" but there is no corresponding "parent close requires all children resolved" boundary.

This ADR defines the parent-close policy, custody transfer protocol, and orphan detection guarantees.

## Decision
We define a **parent-close policy** that every parent workflow must declare at spawn time. When the parent reaches a terminal state, the policy dictates the child resolution action.

### 1. ParentClosePolicy Enum

```rust
/// Policy for resolving child workflows when the parent terminalizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentClosePolicy {
    /// All children must be in a terminal state before the parent may complete.
    /// If any child is active, the parent must wait (backpressure) or fail.
    WaitForChildren,

    /// Children are explicitly terminated when the parent terminalizes.
    /// Termination is graceful: children receive a termination signal,
    /// then a force-kill deadline applies (from ShutdownPropagator).
    TerminateChildren,

    /// Children are transferred to a guardian actor for continued management.
    /// The guardian takes over supervision responsibility.
    /// Used when children have longer-lived business semantics than the parent.
    OrphanGuardian,

    /// Children are abandoned — the parent has no custody obligation.
    /// The children continue running under their own lifecycle until
    /// they terminalize naturally or are externally terminated.
    /// Only permitted for root-level parents (no grandparent).
    Abandon,
}
```

### 2. Parent-Close Evaluation

When a parent workflow enters any `Terminal` substate, the Engine evaluates all tracked children:

```
WHEN ParentWorkflow transitions to Terminal::Completed:
  EVALUATE ParentClosePolicy:
    WaitForChildren → check all_children_terminal()
      IF all terminal → parent transitions to final terminal
      IF any active   → parent enters Terminal::BlockedWaitingChildren

    TerminateChildren → send Terminate to each active child
      → wait for graceful shutdown (graceful_timeout from ShutdownPropagator)
      → force kill remaining after force_kill_timeout

    OrphanGuardian → transfer custody to guardian actor
      → guardian takes ownership of child instance IDs
      → parent may complete once all children transferred

    Abandon → no action (parent completes immediately)
      → only permitted if parent has no grandparent
      → children continue under their own implicit root lifecycle
```

```
WHEN ParentWorkflow transitions to Terminal::Failed:
  EVALUATE ParentClosePolicy:
    WaitForChildren → cascade failure: transition all children to Terminal::Failed
    TerminateChildren → cascade failure: transition all children to Terminal::Failed
    OrphanGuardian → transfer custody to guardian with failure context
    Abandon → cascade failure: transition all children to Terminal::Failed
```

### 3. Typed Boundary: No-Unmanaged-Close

To enforce the "no orphan" invariant at compile time, we add a typed boundary:

```rust
/// Token proving that all children of a parent are resolved.
/// Only constructable when all_children_terminal() is true.
struct AllChildrenResolved;

/// Token proving that custody has been transferred.
struct CustodyTransferred;

struct ParentWorkflow<Policy> {
    policy: Policy,
    // ...
}

impl ParentWorkflow<ParentClosePolicy::WaitForChildren> {
    /// Transitions to terminal only if all children are terminal.
    /// Returns AllChildrenResolved or Err(ActiveChildrenRemaining).
    fn resolve_to_terminal(&self) -> Result<AllChildrenResolved, ActiveChildrenRemaining>;
}

impl ParentWorkflow<ParentClosePolicy::TerminateChildren> {
    /// Initiates child termination and returns a guard.
    /// The guard must be dropped before parent can terminalize.
    fn terminate_children(&mut self) -> ChildTerminationGuard;
}

impl ParentWorkflow<ParentClosePolicy::OrphanGuardian> {
    /// Transfers custody to the guardian.
    /// Returns CustodyTransferred on success.
    fn transfer_to_guardian(&mut self) -> Result<CustodyTransferred, OrphanTransferError>;
}
```

This makes illegal states unrepresentable: a `ParentWorkflow` with `WaitForChildren` policy cannot transition to terminal without first acquiring an `AllChildrenResolved` token.

### 4. Orphan Detection

A background orphan detector runs on a periodic tick (configurable, default 60s):

```
ORPHAN DETECTION LOOP (every 60s):
  FOR EACH root-level workflow:
    FOR EACH child NOT in terminal state:
      IF parent has NO record of this child → flag as orphan
      IF parent has record but child not in ParentChildRegistry → flag as orphan

  ORPHAN RESOLUTION:
    FOR EACH orphaned child:
      IF policy = Abandon → ignore (by definition)
      ELSE → create guardian supervision record
             → notify operator via telemetry event
             → log with full lineage context
```

### 5. Custody Transfer Protocol

When `ParentClosePolicy::OrphanGuardian` triggers, the custody transfer follows this sequence:

1. **Guardian Selection:** The guardian is the root-level supervisor actor (e.g., the `vo-actor` root).
2. **Registry Update:** Child instance IDs are removed from parent's `ParentChildRegistry` and added to the guardian's.
3. **State Preservation:** Child current state, accumulated lifecycle data, and any pending transitions are preserved.
4. **Telemetry:** A `CustodyTransferred` event is emitted with parent lineage, child lineage, and guardian ID.
5. **Boundary Token:** `CustodyTransferred` token is returned, required for parent to proceed to terminal.

### 6. Lifecycle Extension

The `Terminal` superstate in ADR-039 gains two new substates:

- `Terminal::BlockedWaitingChildren` — parent is terminal but waiting for children to resolve (WaitForChildren policy)
- `Terminal::Orphaned` — parent terminated but children remain active (abandoned or orphan-guardian policy, before resolution)

The `Terminal` superstate also gains a `policy` field:

```rust
struct TerminalSubstate {
    variant: TerminalVariant,  // Completed, Failed, Cancelled, Compensated
    policy: ParentClosePolicy,
    children_resolved: bool,
}
```

## Consequences

- **Positive:** No child is ever left in an unmanaged state after parent terminalization.
- **Positive:** The parent-close policy is explicit, configurable per-child-spawn, and visible in the lifecycle model.
- **Positive:** Typed boundaries prevent accidental parent completion with active children.
- **Positive:** The orphan detector provides a safety net for any custody gaps from edge cases (crashes, network partitions).
- **Positive:** Custody transfer preserves child state — children do not lose their lifecycle progress.
- **Negative:** Adds `ParentClosePolicy` as a required field at child spawn time — slightly more ceremony in the spawn API.
- **Negative:** `WaitForChildren` policy introduces a blocking point: the parent cannot complete until children do. This must be documented so workflows don't deadlock with circular dependencies.
- **Negative:** The orphan detector adds a periodic background task with associated overhead (configurable to minimize impact).
- **Negative:** `Abandon` policy creates children that are "unowned" — they must be independently monitorable (e.g., via root-level supervisor or external health checks).
