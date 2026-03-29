# ADR 022 (v2): DAG Cycle Validation and Conditional Outputs

## Status
Accepted

## Context
If a developer creates a cyclic dependency in their graph (`A -> B -> C -> A`), topological sorting will fail. If the Engine discovers this at runtime (when a webhook fires), the workflow fails catastrophically in production. 
Additionally, for branch-conditional DAGs (e.g., Node C depends on either the "Yes" or "No" branch of a Router), the Engine must have deterministic semantics for which parent output is passed to Node C.

## Decision

### 1. Compile-Time/Discovery Validation
The `vo-sdk` must run a cycle detection algorithm (Kahn's or DFS) *before* serializing the DAG during the `--graph` call. 
If a cycle is detected, the SDK prints a fatal error to `stderr` specifying the exact node names forming the cycle and exits with a non-zero code. The Engine treats this as an invalid binary and refuses to register it. Cycles are caught at compile/discovery time, never at runtime.

### 2. Runtime Execution Traversal Semantics
For conditional DAGs, the Engine tracks not just the static graph edges, but the *actually traversed* edges during execution.
When executing Node C, the Engine inspects its incoming edges. It only pipes the JSON output from the specific parent node that was *actually executed* in the current path. Parent nodes on an unexecuted branch are ignored.

## Consequences
- **Positive:** Impossible to deploy an infinitely looping or un-sortable workflow graph.
- **Positive:** Safe, predictable fan-in for conditional branch logic.
- **Negative:** Adds slight computational overhead to the `--graph` execution, though negligible for DAGs under 10,000 nodes.