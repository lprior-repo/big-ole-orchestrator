# ADR 021 (v2): Ghost Workflows and Graph Lifecycle

## Status
Accepted

## Context
When a developer deletes a compiled workflow binary from the `./data/workflows/` directory, the Engine's file watcher detects the deletion. If the Engine simply unregisters the workflow, any active, in-flight instances of that workflow will suddenly have no definition to run against and will permanently stall. 

Conversely, if the Engine ignores the deletion, the workflow remains registered as a "ghost" that accepts new webhook triggers even though the source file is gone.

## Decision
We implement an explicit three-state lifecycle for workflow registrations: **Active**, **Deactivated**, and **Deleted**.

### The Lifecycle State Machine
1. **Registration:** File watcher detects a new binary, calls `--graph`, copies to `/versions/<hash>`, and marks it **Active**. The Engine accepts new triggers.
2. **Deactivation:** File watcher detects binary deletion. The Engine marks the workflow as **Deactivated**. 
   - The Engine instantly stops accepting new instances (`HTTP 404` for triggers).
   - In-flight instances are permitted to continue executing against the pinned version in `/var/wtf/versions/<hash>`.
3. **Garbage Collection (Deletion):** A background Reaper loop occasionally sweeps the `instances` partition. If a Deactivated workflow has exactly `0` running or suspended instances, the Engine physically deletes the `/versions/<hash>` binary and permanently purges the registration from memory.

## Consequences
- **Positive:** Eradicates the "Ghost Workflow" problem.
- **Positive:** Safe, predictable hot-reloading for developers.
- **Negative:** Requires stateful tracking of workflow metadata in `fjall` beyond just the graph definition.